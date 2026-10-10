const { spawn } = require('child_process');
const fs = require('fs');
const path = require('path');
const { targetPath,validateAppId } = require('./validation');
const { EventEmitter } = require('events');
const {DiscordPresence}=require('./discordPresence');
class ProcessManager extends EventEmitter {
  constructor() {
    super();
    this.presence=new DiscordPresence();
    this.nativeDetectionEnabled=true;
    this.currentSession = null; this.child = null; this.pendingChild = null; this.autoStopTimer = null; this.busy = false;
    this.runnerTemplate = path.join(__dirname, '..', 'runner', 'src-win.exe');
    this.gamesRoot = path.join(__dirname, '..', 'games');
    this.historyPath = path.join(__dirname, '..', 'data', 'history.json');
    process.on('exit', () => this.stopSimulationSync());
    for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => { this.stopSimulationSync(); process.exit(0); });
  }
  formatDuration(seconds) { return [Math.floor(seconds/3600), Math.floor(seconds%3600/60), seconds%60].map(n=>String(n).padStart(2,'0')).join(':'); }
  getStatus() {
    if (!this.currentSession) return { active:false, session:null };
    const elapsedSeconds = Math.floor((Date.now()-this.currentSession.startTime)/1000);
    const target = this.currentSession.targetDurationSeconds;
    const remainingSeconds = target ? Math.max(0,target-elapsedSeconds) : null;
    return { active:true, session:{...this.currentSession,presence:this.presence.status(),elapsedSeconds,elapsedFormatted:this.formatDuration(elapsedSeconds),remainingSeconds,remainingFormatted:target?this.formatDuration(remainingSeconds):null,progressPercent:target?Math.min(100,Math.floor(elapsedSeconds/target*100)):null,discordDetected:null} };
  }
  async startSimulation(appId, gameName, executableName, durationMinutes=0, iconHash=null, questId=null, activityEnabled=true) {
    validateAppId(appId);
    const destination = executableName?targetPath(this.gamesRoot,appId,executableName):null;
    const duration = Number(durationMinutes || 0);
    if (!Number.isFinite(duration) || duration < 0 || duration > 1440) throw new Error('Timer must be between 0 and 1440 minutes');
    if (this.busy) throw new Error('Another operation is in progress');
    this.busy=true;
    try {
      await this.stopSimulation('switched_game');
      let child=null;
      if(destination){fs.mkdirSync(path.dirname(destination),{recursive:true});
      fs.copyFileSync(this.runnerTemplate,destination);
      const args=['--title',String(gameName||'Questro')];if(this.nativeDetectionEnabled)args.push('--native-window');
      child=spawn(destination,args,{stdio:'ignore',windowsHide:!this.nativeDetectionEnabled});
      this.pendingChild=child;
      try { await new Promise((resolve,reject)=>{child.once('spawn',resolve);child.once('error',reject);}); }
      finally { if(this.pendingChild===child)this.pendingChild=null; }
      }
      this.child=child;
      this.currentSession={appId,questId,mode:child?(activityEnabled?'process_and_rpc':'process_only'):'rpc_only',gameName:String(gameName||'Game'),executableName:executableName?executableName.replace(/\\/g,'/'):null,executablePath:destination,iconHash,pid:child?.pid||process.pid,startTime:Date.now(),targetDurationSeconds:duration?Math.round(duration*60):null};
      if(child){child.once('exit',()=>{if(this.child===child)this.finishSession(this.stopReason || 'process_exited'); this.stopReason=null;});
      child.once('error',()=>{if(this.child===child)this.finishSession('process_error');});}
      if(duration)this.autoStopTimer=setTimeout(()=>this.stopSimulation('idle_timeout').catch(console.error),Math.round(duration*60000));
      if(activityEnabled)this.presence.start(this.currentSession).catch(()=>{});
      return this.getStatus();
    } finally {this.busy=false;}
  }
  finishSession(reason) {
    this.presence.disconnect();
    clearTimeout(this.autoStopTimer); this.autoStopTimer=null;
    const session=this.currentSession; this.currentSession=null; this.child=null;
    if(!session)return;
    const durationSeconds=Math.floor((Date.now()-session.startTime)/1000);
    this.recordHistory({...session,playedAt:new Date().toISOString(),durationSeconds,durationFormatted:this.formatDuration(durationSeconds),reason});
    this.emit('session-ended', {session, reason});
  }
  async stopSimulation(reason='user_requested') {
    const child=this.child, session=this.currentSession;
    if(!session)return {success:true};
    if(!child){this.finishSession(reason);return {success:true,gameName:session.gameName,reason};}
    this.stopReason=reason;
    await new Promise((resolve,reject)=>{
      if(child.exitCode!==null){resolve();return;}
      const timer=setTimeout(()=>reject(new Error('Process did not stop')),5000);
      child.once('exit',()=>{clearTimeout(timer);resolve();});
      try {child.kill();} catch(e){clearTimeout(timer);reject(e);}
    });
    if(this.currentSession===session)this.finishSession(reason);
    return {success:true,stoppedPid:session.pid,gameName:session.gameName,reason};
  }
  stopSimulationSync(){ clearTimeout(this.autoStopTimer); if(this.pendingChild){try{this.pendingChild.kill();}catch{}this.pendingChild=null;} if(this.child){try{this.child.kill();}catch{}} this.finishSession('app_closed'); }
  recordHistory(entry){try{const history=this.getHistory();history.unshift(entry);fs.mkdirSync(path.dirname(this.historyPath),{recursive:true});fs.writeFileSync(this.historyPath,JSON.stringify({sessions:history.slice(0,50)},null,2));}catch(e){console.warn(e.message);}}
  getHistory(){try{return JSON.parse(fs.readFileSync(this.historyPath,'utf8')).sessions||[];}catch{return [];}}
}
module.exports=new ProcessManager();
