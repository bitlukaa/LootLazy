const {spawn}=require('node:child_process'),fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),readline=require('node:readline');
const root=path.resolve(__dirname,'..'),engine=path.join(root,'data/steam-engine/LootLazy.Steam.exe'),fixture=fs.mkdtempSync(path.join(root,'data/accounts-protocol-'));
let counter=0;const all=[];
async function launch(parentId=process.pid){
 const child=spawn(engine,['--offline-test',fixture,String(parentId)],{windowsHide:true,stdio:['pipe','pipe','pipe'],env:{...process.env,DOTNET_BUNDLE_EXTRACT_BASE_DIR:path.join(root,'data/engine-test-runtime')}});all.push(child);
 const queued=[],waiting=[];let exited=false;
 child.on('error',error=>waiting.splice(0).forEach(({reject})=>reject(error)));
 child.on('exit',()=>{exited=true;waiting.splice(0).forEach(({reject})=>reject(Error('Engine exited before response')));});
 readline.createInterface({input:child.stdout}).on('line',line=>{const value=JSON.parse(line);if(waiting.length)waiting.shift().resolve(value);else queued.push(value);});
 function next(){if(queued.length)return Promise.resolve(queued.shift());if(exited)return Promise.reject(Error('Engine exited'));return new Promise((resolve,reject)=>{const timeout=setTimeout(()=>reject(Error('Engine timeout')),60000);waiting.push({resolve:value=>{clearTimeout(timeout);resolve(value);},reject:error=>{clearTimeout(timeout);reject(error);}});});}
 assert.deepEqual(await next(),{ready:true,protocol:1});
 return {child,async call(command,body={}){const id=++counter;child.stdin.write(JSON.stringify({id,protocol:1,command,body})+'\n');const result=await next();assert.equal(result.id,id);return result;},async raw(frame){child.stdin.write(frame+'\n');return next();},async close(){const done=new Promise(resolve=>child.once('exit',resolve));child.stdin.end();await done;}};
}
(async()=>{
 const fixtureQr=JSON.parse(require('child_process').execFileSync(engine,['--qr-fixture'],{windowsHide:true,env:{...process.env,DOTNET_BUNDLE_EXTRACT_BASE_DIR:path.join(root,'data/engine-test-runtime')}}));assert.deepEqual(fixtureQr,JSON.parse(fs.readFileSync(path.join(root,'tests/fixtures/steam-auth-qr.json'),'utf8')));
 let run=await launch();assert.equal((await run.raw('not-json')).error,'accounts_failed');assert.equal((await run.raw(JSON.stringify({id:99,protocol:2,command:'add',body:{label:'Rejected',login:'reject_login'}}))).error,'accounts_protocol');assert.deepEqual((await run.call('status')).result.accounts,[]);
 for(const body of [{label:'Test',login:'../../outside'},{label:'',login:'safe_login'},{label:'x'.repeat(49),login:'safe_login'}])assert.equal((await run.call('add',body)).error,'accounts_invalid');
 assert.equal((await run.call('add',{label:'No login',login:''})).error,'accounts_invalid');
 const qrAdded=await run.call('add',{label:'QR pending',login:'',method:'qr'});assert.equal(qrAdded.ok,true);const qrId=qrAdded.result.accounts[0].id;
 assert.equal((await run.call('qr',{id:qrId})).error,'accounts_not_ready');assert.equal((await run.call('remove',{id:qrId})).ok,true);
 const pending=await run.call('add',{label:'Pending must not persist',login:'',method:'qr',draft:true});const pendingId=pending.result.accounts[0].id;assert.equal(pending.result.accounts[0].draft,true);assert.ok(!fs.readFileSync(path.join(fixture,'accounts.json'),'utf8').includes(pendingId));assert.equal((await run.call('commit',{id:pendingId,label:'Not authenticated'})).error,'accounts_not_ready');
 await run.close();run=await launch();assert.deepEqual((await run.call('status')).result.accounts,[]);assert.equal(fs.existsSync(path.join(fixture,'config',pendingId+'.json')),false,'Abandoned draft config is removed on restart');
 let result=await run.call('add',{label:'Account one',login:'test_login'});assert.equal(result.ok,true);const id=result.result.accounts[0].id;assert.match(id,/^ll_[a-f0-9]{32}$/);assert.equal(result.result.accounts[0].rememberLogin,false,'An unverified profile is not remembered for login');
 for(const presence of ['online','away','invisible','offline']){const changed=await run.call('presence',{id,presence});assert.equal(changed.ok,true);assert.equal(changed.result.accounts[0].presence,presence);assert.deepEqual(changed.result.accounts[0].games,[]);}
 assert.equal((await run.call('presence',{id,presence:'busy'})).error,'accounts_invalid');
 assert.equal((await run.call('add',{label:'Duplicate',login:'TEST_LOGIN'})).error,'accounts_duplicate');
 result=await run.call('games',{id,games:[570,730,570]});assert.deepEqual(result.result.accounts[0].games,[570,730]);
 for(const games of [[0],[-1],[1.5],['730'],[null],Array.from({length:33},(_,i)=>i+1)])assert.equal((await run.call('games',{id,games})).error,'accounts_invalid');assert.deepEqual((await run.call('status')).result.accounts[0].games,[570,730],'Rejected input leaves the saved games unchanged');
 assert.equal((await run.call('games',{id:'../../outside',games:[570]})).error,'accounts_missing');
 fs.mkdirSync(path.join(fixture,'accounts.json.new'));assert.equal((await run.call('games',{id,games:[440]})).ok,false);assert.deepEqual((await run.call('status')).result.accounts[0].games,[570,730],'A failed disk save does not change in-memory selection');fs.rmdirSync(path.join(fixture,'accounts.json.new'));
 assert.equal((await run.call('command',{id,value:'!anything'})).error,'accounts_invalid');
 assert.equal((await run.call('signin',{id,password:'fixture-password-never-store'})).error,'accounts_not_ready');
 assert.ok(!fs.readFileSync(path.join(fixture,'accounts.json'),'utf8').includes('fixture-password'));
 assert.ok(!fs.readFileSync(path.join(fixture,'config',id+'.json'),'utf8').includes('SteamPassword'));
 const config=JSON.parse(fs.readFileSync(path.join(fixture,'config',id+'.json'),'utf8'));assert.equal(config.Enabled,false);assert.equal(config.RemoteCommunication,0);assert.equal(config.TradeCheckPeriod,0);assert.equal(config.FarmingPreferences,9);
 await run.close();run=await launch();result=await run.call('status');assert.deepEqual(result.result.accounts[0].games,[570,730]);assert.equal(result.result.active,false);assert.equal(result.result.accounts[0].presence,'offline','Appearance is saved across restart');assert.equal(result.result.accounts[0].rememberLogin,false,'Explicit saved-login preference survives restart');
 // Older account files have no presence field; they retain the original Online default.
 await run.close();const legacy=JSON.parse(fs.readFileSync(path.join(fixture,'accounts.json'),'utf8'));delete legacy[0].presence;delete legacy[0].rememberLogin;fs.writeFileSync(path.join(fixture,'accounts.json'),JSON.stringify(legacy));run=await launch();assert.equal((await run.call('status')).result.accounts[0].presence,'online');assert.equal((await run.call('status')).result.accounts[0].rememberLogin,true,'Existing account metadata opts into restoring the protected saved login');
 assert.equal((await run.call('remove',{id})).ok,true);assert.deepEqual((await run.call('status')).result.accounts,[]);assert.equal(fs.existsSync(path.join(fixture,'config',id+'.json')),false);
 for(let index=0;index<20;index++)assert.equal((await run.call('add',{label:'Capacity '+index,login:'capacity_'+index})).ok,true);
 assert.equal((await run.call('add',{label:'Overflow',login:'capacity_overflow'})).error,'accounts_invalid');
 assert.equal((await run.call('status')).result.accounts.length,20);
 await run.close();
 const owner=spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{windowsHide:true,stdio:'ignore'});all.push(owner);
 run=await launch(owner.pid);const exited=new Promise(resolve=>run.child.once('exit',resolve));owner.kill();const timeout=new Promise((_,reject)=>setTimeout(()=>reject(Error('Parent ownership shutdown timed out')),10000).unref());await Promise.race([exited,timeout]);
 console.log('Account protocol: validation, isolation, persistence, EOF shutdown, feature restrictions and secret-absence checks passed.');
})().catch(error=>{console.error(error.message);process.exitCode=1;}).finally(()=>{for(const child of all)if(child.exitCode===null)child.kill();});
