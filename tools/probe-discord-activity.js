// Probe only the authorized AION 2 activity; never request account data or tokens.
const fs=require('fs'),path=require('path'),net=require('net'),os=require('os');
const {spawn}=require('child_process');
const root=path.resolve(__dirname,'..');
const fixture=fs.mkdtempSync(path.join(root,'data/.test-temp/lootlazy-live-aion-'));
const exe=path.join(fixture,'Aion2/Binaries/Win64/AION2.exe');fs.mkdirSync(path.dirname(exe),{recursive:true});fs.copyFileSync(path.join(root,'runner/src-win.exe'),exe);
const helper=spawn(exe,['--title','AION 2','--native-window','--parent',String(process.pid)],{windowsHide:true,stdio:'ignore'});
const frame=(op,payload)=>{const body=Buffer.from(JSON.stringify(payload)),head=Buffer.alloc(8);head.writeUInt32LE(op,0);head.writeUInt32LE(body.length,4);return Buffer.concat([head,body]);};
async function probe(pid){
 for(let index=0;index<10;index++){
  const result=await new Promise(resolve=>{
   const socket=net.createConnection('\\\\?\\pipe\\discord-ipc-'+index);let buffer=Buffer.alloc(0),ready=false,done=false;
   const finish=value=>{if(done)return;done=true;clearTimeout(deadline);socket.destroy();resolve(value);};
   const deadline=setTimeout(()=>finish({connected:ready,state:'timeout',pid}),5000);
   socket.on('error',()=>finish(null));socket.on('connect',()=>socket.write(frame(0,{v:1,client_id:'1445982809792708628'})));
   socket.on('data',chunk=>{
    buffer=Buffer.concat([buffer,chunk]);
    while(buffer.length>=8){const op=buffer.readUInt32LE(0),length=buffer.readUInt32LE(4);if(length>1024*1024)return finish({state:'oversized_frame'});if(buffer.length<8+length)return;const payload=JSON.parse(buffer.subarray(8,8+length));buffer=buffer.subarray(8+length);
     if(op===3){socket.write(frame(4,payload));continue;}
     if(op===2)return finish({connected:ready,state:'closed',pid});
     if(op===1&&payload.evt==='READY'){ready=true;socket.write(frame(1,{cmd:'SET_ACTIVITY',nonce:'lootlazy-aion-probe',args:{pid,activity:{type:0,details:'Playing AION 2',timestamps:{start:Math.floor(Date.now()/1000)}}}}));}
     else if(op===1&&payload.nonce==='lootlazy-aion-probe'){
      if(payload.evt==='ERROR')return finish({connected:ready,state:'rejected',pid,errorCode:payload.data?.code,errorMessage:payload.data?.message});
      const value={connected:ready,state:'acknowledged',pid,applicationId:payload.data?.application_id,activityName:payload.data?.name,type:payload.data?.type};
      setTimeout(()=>{socket.write(frame(1,{cmd:'SET_ACTIVITY',nonce:'lootlazy-aion-clear',args:{pid,activity:null}}));finish(value);},1500);
     }
    }
   });
  });
  if(result)return result;
 }
 return {state:'desktop_unavailable',pid};
}
(async()=>{try{
 const results={testedAt:new Date().toISOString(),game:'AION 2',appId:'1445982809792708628',helperPid:helper.pid,parentPid:process.pid,helper:await probe(helper.pid),parent:await probe(process.pid),visibleDetectionVerified:false};
 fs.mkdirSync(path.join(root,'.internal'),{recursive:true});fs.writeFileSync(path.join(root,'.internal/AION2_RPC_PROBE.json'),JSON.stringify(results,null,2));console.log(JSON.stringify(results));
}finally{helper.kill();await new Promise(resolve=>{if(helper.exitCode!==null)return resolve();helper.once('exit',resolve);setTimeout(resolve,3000);});fs.rmSync(fixture,{recursive:true,force:true});}})().catch(error=>{console.error(error.message);process.exitCode=1;});
