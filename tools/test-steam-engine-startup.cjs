const {spawn}=require('node:child_process'),fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),readline=require('node:readline');
const root=path.resolve(__dirname,'..'),fixture=fs.mkdtempSync(path.join(root,'data/accounts-startup-')),engine=path.join(root,'data/steam-engine/LootLazy.Steam.exe');const children=[];
async function launch(){
 const child=spawn(engine,['--bridge',fixture,String(process.pid)],{windowsHide:true,stdio:['pipe','pipe','pipe'],env:{...process.env,DOTNET_BUNDLE_EXTRACT_BASE_DIR:path.join(root,'data/engine-test-runtime')}});children.push(child);
 const queue=[],pending=[];let dead=false,sequence=0;
 child.once('exit',()=>{dead=true;for(const waiter of pending.splice(0))waiter.reject(Error('Engine exited before response'));});
 readline.createInterface({input:child.stdout}).on('line',line=>{const result=JSON.parse(line);if(pending.length)pending.shift().resolve(result);else queue.push(result);});
 function next(){if(queue.length)return Promise.resolve(queue.shift());if(dead)return Promise.reject(Error('Engine exited'));return new Promise((resolve,reject)=>{const timer=setTimeout(()=>reject(Error('Engine startup timeout')),100000);pending.push({resolve:value=>{clearTimeout(timer);resolve(value);},reject:error=>{clearTimeout(timer);reject(error);}});});}
 assert.deepEqual(await next(),{ready:true,protocol:1});
 return {async call(command,body={}){child.stdin.write(JSON.stringify({id:++sequence,protocol:1,command,body})+'\n');const response=await next();assert.equal(response.ok,true,response.error);return response.result;},async close(){const exited=new Promise(resolve=>child.once('exit',resolve));child.stdin.end();assert.equal(await exited,0);}};
}
(async()=>{
 let run=await launch();assert.deepEqual((await run.call('status')).accounts,[]);
 const added=await run.call('add',{label:'Offline fixture',login:'lootlazy_offline_fixture'}),id=added.accounts[0].id;assert.equal(added.accounts[0].active,false,'Registration must not authenticate automatically');
 assert.deepEqual((await run.call('remove',{id})).accounts,[]);await run.close();
 const qrRun=await launch();const qr=await qrRun.call('add',{label:'QR pending fixture',login:'',method:'qr'});assert.equal(qr.accounts[0].error,null);assert.equal(qr.accounts[0].active,false);assert.equal((await qrRun.call('qr',{id:qr.accounts[0].id})).rows,null);await qrRun.call('remove',{id:qr.accounts[0].id});await qrRun.close();
 const brokenId='ll_'+'f'.repeat(32);fs.writeFileSync(path.join(fixture,'accounts.json'),JSON.stringify([{id:brokenId,label:'Recovery fixture',login:'lootlazy_recovery_fixture',games:[]}]))
 for(const suffix of ['.db','.db.new'])fs.writeFileSync(path.join(fixture,'config',brokenId+suffix),'unusable-fixture-database');
 run=await launch();const status=await run.call('status');assert.equal(status.accounts[0].active,false);assert.equal(status.accounts[0].error,'accounts_restore_failed');
 assert.deepEqual((await run.call('remove',{id:brokenId})).accounts,[]);for(const suffix of ['.json','.db','.db.new'])assert.equal(fs.existsSync(path.join(fixture,'config',brokenId+suffix)),false);
 await run.close();
 // A synthetic protected login can be loaded after an app replacement without contacting Steam.
 const savedId='ll_'+'e'.repeat(32),base64=value=>Buffer.from(JSON.stringify(value)).toString('base64url');
 const token=base64({alg:'none',typ:'JWT'})+'.'+base64({sub:'76561198000000000',exp:Math.floor(Date.now()/1000)+86400})+'.fixture-signature';
 const protectScript='Add-Type -AssemblyName System.Security; $taskBytes=[Text.Encoding]::UTF8.GetBytes([Console]::In.ReadToEnd()); [Console]::Write("LootLazy-DPAPI-v1:"+[Convert]::ToBase64String([Security.Cryptography.ProtectedData]::Protect($taskBytes,$null,[Security.Cryptography.DataProtectionScope]::CurrentUser)))';
 const protectedDb=require('node:child_process').execFileSync('powershell',['-NoProfile','-Command',protectScript],{input:JSON.stringify({BackingRefreshToken:token}),encoding:'utf8',windowsHide:true});
 fs.writeFileSync(path.join(fixture,'accounts.json'),JSON.stringify([{id:savedId,label:'Remembered fixture',login:'lootlazy_saved_fixture',games:[730],rememberLogin:false}]));
 fs.writeFileSync(path.join(fixture,'config',savedId+'.db'),protectedDb);
 run=await launch();let saved=(await run.call('status')).accounts[0];assert.equal(saved.canResume,true,'The protected saved login is actually loaded by the engine');assert.equal(saved.active,false,'Explicit Sign out prevents automatic login');assert.deepEqual(saved.games,[730]);
 await run.call('disconnect',{id:savedId,signOut:true});await run.close();
 run=await launch();saved=(await run.call('status')).accounts[0];assert.equal(saved.canResume,true,'Sign out retains the protected login for a later explicit sign-in');assert.equal(saved.active,false);assert.equal(saved.rememberLogin,false);assert.equal(fs.readFileSync(path.join(fixture,'config',savedId+'.db'),'utf8'),protectedDb);await run.call('remove',{id:savedId});await run.close();
 console.log('Actual engine startup, protected saved-login loading, explicit sign-out persistence, unreadable-login recovery and complete removal passed; no Steam authentication performed.');
})().catch(error=>{console.error(error.message);process.exitCode=1;}).finally(()=>{for(const child of children)if(child.exitCode===null)child.kill();});
