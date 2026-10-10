process.env.QUESTRA_ARTWORK_OFFLINE='1';
const fs=require('fs');const path=require('path');const assert=require('node:assert/strict');
process.env.QUESTRA_PORT='3867';const root=process.env.QUESTRA_TEST_ROOT||path.resolve(__dirname,'..');
const backups=['queue.json','history.json'].map(name=>{const file=path.join(root,'data',name);return {file,bytes:fs.existsSync(file)?fs.readFileSync(file):null};});
const server=require(path.join(root,'src/server'));const manager=require(path.join(root,'src/processManager'));
const api=async(route,body)=>{const response=await fetch('http://127.0.0.1:3867'+route,body===undefined?{}:{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});const data=await response.json();if(!response.ok)throw new Error(data.error||'HTTP '+response.status);return data;};
(async()=>{try{
 await server.start();const games=(await api('/api/games?limit=2')).games;
 await api('/api/queue/items',{action:'clear'});
 await api('/api/queue/items',{action:'mode',automatic:true});
 await api('/api/queue/items',{action:'add',appId:games[0].id,executableName:'queue-check-one.exe',durationMinutes:1});
 await api('/api/queue/items',{action:'add',appId:games[1].id,executableName:'queue-check-two.exe',durationMinutes:1});
 const started=await api('/api/queue/start',{});assert.equal(started.queue.phase,'running');console.log('Real timed queue: first owned process running for one minute.');
 let transition=null,endedAt=null,second=null;const deadline=Date.now()+85000;
 while(Date.now()<deadline){const queue=await api('/api/queue');if(queue.phase==='error')throw new Error(queue.error);if(queue.phase==='waiting'&&!transition){assert.equal(manager.getStatus().active,false);endedAt=Date.parse(manager.getHistory()[0].playedAt);transition={waitUntil:queue.waitUntil,gapMilliseconds:queue.waitUntil-endedAt};assert.ok(transition.gapMilliseconds>=4990&&transition.gapMilliseconds<=10010);console.log('First process stopped; scheduled gap: '+transition.gapMilliseconds+' ms.');}if(queue.phase==='running'&&queue.index===1){second=manager.getStatus().session;break;}await new Promise(r=>setTimeout(r,250));}
 assert.ok(transition,'Timed transition observed');assert.ok(second,'Second process started');assert.equal(second.executableName,'queue-check-two.exe');assert.ok(second.startTime>=transition.waitUntil);assert.ok(second.startTime-endedAt<12000);
 await api('/api/queue/cancel',{});assert.equal(manager.getStatus().active,false);
 fs.writeFileSync(path.join(__dirname,'../docs/QUEUE_TIMING.json'),JSON.stringify({passed:true,firstDurationSeconds:60,scheduledGapMilliseconds:transition.gapMilliseconds,observedGapMilliseconds:second.startTime-endedAt,secondStartedAfterFirstStopped:true,cancellationStoppedSecond:true},null,2));console.log('Real timed transition passed.');
}finally{await server.queueManager.cancel();server.queueManager.dispose();await manager.stopSimulation();await new Promise(r=>server.server.close(r));for(const {file,bytes}of backups){if(bytes)fs.writeFileSync(file,bytes);else if(fs.existsSync(file))fs.unlinkSync(file);}}
})().catch(e=>{console.error(e);process.exitCode=1});
