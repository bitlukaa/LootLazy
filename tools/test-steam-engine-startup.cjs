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
 const brokenId='ll_'+'f'.repeat(32);fs.writeFileSync(path.join(fixture,'accounts.json'),JSON.stringify([{id:brokenId,label:'Recovery fixture',login:'lootlazy_recovery_fixture',games:[]}]))
 for(const suffix of ['.db','.db.new'])fs.writeFileSync(path.join(fixture,'config',brokenId+suffix),'unusable-fixture-database');
 run=await launch();const status=await run.call('status');assert.equal(status.accounts[0].active,false);assert.equal(status.accounts[0].error,'accounts_restore_failed');
 assert.deepEqual((await run.call('remove',{id:brokenId})).accounts,[]);for(const suffix of ['.json','.db','.db.new'])assert.equal(fs.existsSync(path.join(fixture,'config',brokenId+suffix)),false);
 await run.close();console.log('Actual engine startup, disabled bot lifecycle, unreadable-login recovery and complete account removal passed; no Steam authentication performed.');
})().catch(error=>{console.error(error.message);process.exitCode=1;}).finally(()=>{for(const child of children)if(child.exitCode===null)child.kill();});
