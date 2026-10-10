// Local Discord IPC transport. No user tokens, account APIs or client injection.
const net=require('net');
const {randomUUID}=require('crypto');
function frame(opcode,payload){const body=Buffer.from(JSON.stringify(payload),'utf8'),header=Buffer.alloc(8);header.writeUInt32LE(opcode);header.writeUInt32LE(body.length,4);return Buffer.concat([header,body]);}
class FrameReader{
 constructor(onFrame){this.buffer=Buffer.alloc(0);this.onFrame=onFrame;}
 push(chunk){this.buffer=Buffer.concat([this.buffer,chunk]);while(this.buffer.length>=8){const opcode=this.buffer.readUInt32LE(0),length=this.buffer.readUInt32LE(4);if(length>1024*1024)throw new Error('IPC frame exceeds limit');if(this.buffer.length<8+length)return;const bytes=this.buffer.subarray(8,8+length);this.buffer=this.buffer.subarray(8+length);this.onFrame(opcode,JSON.parse(bytes.toString('utf8')));}}
}
class DiscordPresence{
 constructor(options={}){this.paths=options.paths||Array.from({length:10},(_,n)=>'\\\\?\\pipe\\discord-ipc-'+n);this.timeout=options.timeout||2500;this.socket=null;this.pending=new Map();this.generation=0;this.retry=null;this.session=null;this.result={state:'idle',activityConfirmed:false,errorCode:null};}
 status(){return {...this.result};}
 write(opcode,payload){if(!this.socket||this.socket.destroyed)throw new Error('IPC disconnected');this.socket.write(frame(opcode,payload));}
 command(payload){return new Promise((resolve,reject)=>{const nonce=randomUUID(),timer=setTimeout(()=>{this.pending.delete(nonce);reject(new Error('IPC acknowledgement timeout'));},this.timeout);this.pending.set(nonce,{resolve,reject,timer});try{this.write(1,{...payload,nonce});}catch(error){clearTimeout(timer);this.pending.delete(nonce);reject(error);}});}
 closeSocket(){const socket=this.socket;this.socket=null;for(const p of this.pending.values()){clearTimeout(p.timer);p.reject(new Error('IPC disconnected'));}this.pending.clear();if(socket)socket.destroy();}
 disconnect(){++this.generation;clearTimeout(this.retry);this.retry=null;this.session=null;this.closeSocket();this.result={state:'idle',activityConfirmed:false,errorCode:null};}
 async start(session){this.disconnect();this.session=session;const generation=this.generation;await this.connect(generation);}
 scheduleRetry(generation){if(generation!==this.generation||!this.session)return;clearTimeout(this.retry);this.retry=setTimeout(()=>this.connect(generation).catch(()=>{}),10000);this.retry.unref?.();}
 async connect(generation){
  this.result={state:'connecting',activityConfirmed:false,errorCode:null};let connected=false;
  for(const pipe of this.paths){if(generation!==this.generation)return;try{await this.open(pipe,generation);connected=true;break;}catch{if(generation!==this.generation)return;this.closeSocket();}}
  if(generation!==this.generation)return;
  if(!connected){this.result={state:'unavailable',activityConfirmed:false,errorCode:null};this.scheduleRetry(generation);return;}
  try{await this.command({cmd:'SET_ACTIVITY',args:{pid:process.pid,activity:{type:0,details:'Playing '+this.session.gameName,timestamps:{start:Math.floor(this.session.startTime/1000)}}}});if(generation===this.generation)this.result={state:'ready',activityConfirmed:true,errorCode:null};}
  catch(error){if(generation!==this.generation)return;this.result={state:'rejected',activityConfirmed:false,errorCode:error.code||null};this.closeSocket();this.scheduleRetry(generation);}
 }
 open(pipe,generation){return new Promise((resolve,reject)=>{
  const socket=net.createConnection(pipe);this.socket=socket;let ready=false,settled=false;
  const timer=setTimeout(()=>{if(!settled){settled=true;reject(new Error('IPC handshake timeout'));socket.destroy();}},this.timeout);
  const fail=error=>{if(!settled){settled=true;clearTimeout(timer);reject(error);}};
  const reader=new FrameReader((opcode,payload)=>{
   if(generation!==this.generation)return;
   if(opcode===3){this.write(4,payload);return;}
   if(opcode===2){fail(new Error('IPC closed'));socket.destroy();return;}
   if(opcode!==1)return;
   if(payload.evt==='READY'&&!ready){ready=true;settled=true;clearTimeout(timer);resolve();return;}
   const pending=this.pending.get(payload.nonce);if(!pending)return;clearTimeout(pending.timer);this.pending.delete(payload.nonce);
   if(payload.evt==='ERROR'){const error=new Error('Discord rejected activity');error.code=payload.data?.code;pending.reject(error);}else if(payload.cmd==='SET_ACTIVITY')pending.resolve();else pending.reject(new Error('Unexpected acknowledgement'));
  });
  socket.on('connect',()=>{if(generation===this.generation)this.write(0,{v:1,client_id:this.session.appId});});
  socket.on('data',chunk=>{try{reader.push(chunk);}catch(error){fail(error);socket.destroy();}});
  socket.on('error',fail);socket.on('close',()=>{fail(new Error('IPC disconnected'));if(this.socket===socket){this.closeSocket();if(generation===this.generation&&this.session){this.result={state:'disconnected',activityConfirmed:false,errorCode:null};this.scheduleRetry(generation);}}});
 });}
}
module.exports={DiscordPresence,FrameReader,frame};
