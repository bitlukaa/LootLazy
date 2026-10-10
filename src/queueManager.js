const fs = require('fs');
const path = require('path');
const { randomInt, randomUUID } = require('crypto');
const { validateAppId, validateExecutable } = require('./validation');
const ACTIVE_PHASES = new Set(['starting','running','waiting','ready_next','stopping']);
class QueueManager {
  constructor(manager, options = {}) {
    this.manager = manager;
    this.file = options.file === undefined ? path.resolve(__dirname,'../data/queue.json') : options.file;
    this.gap = options.gap || (() => randomInt(5,11));
    this.now = options.now || Date.now;
    this.schedule = options.schedule || setTimeout;
    this.clear = options.clear || clearTimeout;
    this.items = []; this.phase = 'idle'; this.index = -1; this.generation = 0;
    this.timer = null; this.waitUntil = null; this.ownedPid = null; this.launchTask = null; this.error = null;
    this.automatic = options.automatic === true;
    this.load();
    this.onEnded = event => this.handleEnded(event);
    manager.on('session-ended',this.onEnded);
  }
  validate(item) {
    validateAppId(item.appId); const executableName = item.executableName?validateExecutable(item.executableName):null;
    const durationMinutes = Number(item.durationMinutes);
    if (!Number.isFinite(durationMinutes) || durationMinutes < 1 || durationMinutes > 1440) throw new Error('Queue duration must be 1–1440 minutes');
    return {id:randomUUID(),appId:item.appId,questId:item.questId||null,activityEnabled:item.activityEnabled!==false,gameName:String(item.gameName || 'Game').slice(0,200),executableName,durationMinutes,iconHash:item.iconHash || null,state:'queued'};
  }
  load() {
    if (!this.file) return;
    try { const data=JSON.parse(fs.readFileSync(this.file,'utf8')); this.items=(Array.isArray(data.items)?data.items:[]).slice(0,30).map(item=>this.validate(item)); this.automatic=data.automatic===true; } catch { this.items=[]; }
  }
  save() {
    if (!this.file) return;
    fs.mkdirSync(path.dirname(this.file),{recursive:true});
    const items=this.items.map(({state,...item})=>item);
    const temporary=this.file+'.tmp'; fs.writeFileSync(temporary,JSON.stringify({items,automatic:this.automatic},null,2)); fs.renameSync(temporary,this.file);
  }
  get active() { return ACTIVE_PHASES.has(this.phase); }
  getStatus() {
    return {active:this.active,phase:this.phase,automatic:this.automatic,index:this.index,error:this.error,waitUntil:this.waitUntil,waitSecondsRemaining:this.waitUntil?Math.max(0,Math.ceil((this.waitUntil-this.now())/1000)):null,items:this.items.map(item=>({...item}))};
  }
  edit(action, payload = {}) {
    if(this.active){
      const index=this.items.findIndex(item=>item.id===payload.id);
      if(index<=this.index||this.items[index]?.state!=='queued'||!['remove','move','duration'].includes(action))throw new Error('Only upcoming games can be edited while running');
      if(action==='remove')this.items.splice(index,1);
      else if(action==='duration')this.items[index].durationMinutes=this.validate({...this.items[index],durationMinutes:payload.durationMinutes}).durationMinutes;
      else{const next=index+(payload.direction==='up'?-1:payload.direction==='down'?1:0);if(next===index||next<=this.index||next>=this.items.length)throw new Error('Invalid upcoming position');[this.items[index],this.items[next]]=[this.items[next],this.items[index]];}
      this.save();return this.getStatus();
    }
    if (action==='mode')this.automatic=payload.automatic===true;
    else if (action==='add') { if(this.items.length>=30)throw new Error('Maximum 30 queue entries'); this.items.push(this.validate(payload)); }
    else if(action==='clear') this.items=[];
    else {
      const index=this.items.findIndex(item=>item.id===payload.id); if(index<0)throw new Error('Queue item not found');
      if(action==='remove')this.items.splice(index,1);
      else if(action==='move') { const next=index+(payload.direction==='up'?-1:payload.direction==='down'?1:0); if(next===index)throw new Error('Invalid move'); if(next>=0&&next<this.items.length)[this.items[index],this.items[next]]=[this.items[next],this.items[index]]; }
      else if(action==='duration')this.items[index].durationMinutes=this.validate({...this.items[index],durationMinutes:payload.durationMinutes}).durationMinutes;
      else throw new Error('Invalid queue action');
    }
    this.phase='idle'; this.index=-1; this.error=null; this.items.forEach(item=>item.state='queued'); this.save(); return this.getStatus();
  }
  async start() {
    if(this.active)throw new Error('Queue is already running');
    if(this.manager.getStatus().active || this.manager.busy)throw new Error('Stop the current game before starting the queue');
    if(!this.items.length)throw new Error('Queue is empty');
    this.index=0; this.error=null; this.items.forEach(item=>item.state='queued');
    const token=++this.generation; await this.launch(token); return this.getStatus();
  }
  async appendQuest(payload) {
    if(this.active&&this.items.some(item=>!item.questId))throw new Error('Another queue is running');
    if(this.items.some(item=>item.questId===payload.questId&&['queued','starting','running','waiting','ready_next'].includes(item.state)))return this.getStatus();
    const item=this.validate(payload);
    if(this.active){if(this.items.length>=30)throw new Error('Maximum 30 queue entries');this.items.push(item);return this.getStatus();}
    this.items=[item];this.automatic=true;return this.start();
  }
  async playOrAppend(payload){
    const item=this.validate(payload);
    if(this.active){if(this.items.length>=30)throw new Error('Maximum 30 selected games');if(this.items.slice(this.index).some(current=>current.appId===item.appId&&current.executableName===item.executableName&&['queued','starting','running','waiting'].includes(current.state)))return this.getStatus();this.items.push(item);this.automatic=true;this.save();return this.getStatus();}
    if(this.manager.getStatus().active||this.manager.busy)throw new Error('Stop the current session first');
    this.items=[item];this.automatic=true;this.save();return this.start();
  }
  launch(token) { this.launchTask=this.performLaunch(token); return this.launchTask; }
  async performLaunch(token) {
    if(token!==this.generation)return;
    const item=this.items[this.index]; this.phase='starting'; item.state='starting'; this.waitUntil=null;
    try {
      const result=await this.manager.startSimulation(item.appId,item.gameName,item.executableName,item.durationMinutes,item.iconHash,item.questId,item.activityEnabled);
      if(token!==this.generation) {
        if(this.manager.getStatus().session?.pid===result.session?.pid)await this.manager.stopSimulation('queue_cancelled');
        return;
      }
      if(!result.active || !result.session)throw new Error('Queue process failed to start');
      this.ownedPid=result.session.pid; item.state='running'; this.phase='running';
    } catch(error) {
      if(token!==this.generation)return;
      this.phase='error'; this.error='launch_failed'; item.state='error'; this.ownedPid=null;
      console.error('Queue launch failed:',error.message);
    }
  }
  handleEnded({session,reason}) {
    if(this.phase!=='running' || session.pid!==this.ownedPid)return;
    this.ownedPid=null;
    if(reason!=='idle_timeout'&&reason!=='quest_cancelled'&&reason!=='item_cancelled') {
      this.items[this.index].state='error'; this.error='process_ended_early'; this.phase='error'; return;
    }
    if(reason==='item_cancelled'){this.items.splice(this.index,1);this.index--;this.save();}else this.items[this.index].state=reason==='idle_timeout'?'completed':'cancelled';
    this.advanceAfterCurrent();
  }
  advanceAfterCurrent(){
    let next=this.index+1;while(next<this.items.length&&this.items[next].state==='cancelled')next++;
    if(next>=this.items.length){this.phase='completed';this.waitUntil=null;return;}
    this.index=next; this.phase='waiting'; this.items[this.index].state='waiting';
    const seconds=this.gap();
    if(!Number.isInteger(seconds)||seconds<5||seconds>10){this.phase='error';this.error='invalid_gap';return;}
    this.waitUntil=this.now()+seconds*1000; const token=this.generation;
    this.timer=this.schedule(()=>{this.timer=null;if(token!==this.generation)return;if(this.automatic)this.launch(token);else{this.phase='ready_next';this.items[this.index].state='ready_next';this.waitUntil=null;}},seconds*1000);
  }
  async cancelQuest(questId){
    const item=this.items.find(item=>item.questId===questId&&['queued','starting','running','waiting','ready_next'].includes(item.state));
    if(!item||!this.active)return this.getStatus();
    if(this.items.indexOf(item)!==this.index){item.state='cancelled';return this.getStatus();}
    if(this.phase==='starting'&&this.launchTask)await this.launchTask;
    if(this.items[this.index]!==item)return this.getStatus();
    if(this.phase==='running'&&this.manager.getStatus().session?.pid===this.ownedPid){await this.manager.stopSimulation('quest_cancelled');return this.getStatus();}
    if(this.phase==='waiting'||this.phase==='ready_next'){++this.generation;this.clear(this.timer);this.timer=null;this.waitUntil=null;item.state='cancelled';this.advanceAfterCurrent();}
    return this.getStatus();
  }
  async next() {
    if(this.phase!=='ready_next')throw new Error('Next game is not ready');
    if(this.manager.getStatus().active)throw new Error('A game is already running');
    await this.launch(this.generation);return this.getStatus();
  }
  async stopCurrent(){if(this.phase==='running'&&this.manager.getStatus().session?.pid===this.ownedPid)await this.manager.stopSimulation('item_cancelled');return this.getStatus();}
  async cancel() {
    if(!this.active)return this.getStatus();
    ++this.generation; this.clear(this.timer); this.timer=null; this.waitUntil=null; this.phase='stopping';
    const owned=this.ownedPid; this.ownedPid=null;
    if(this.launchTask)await this.launchTask;
    try {
      if(owned && this.manager.getStatus().session?.pid===owned)await this.manager.stopSimulation('queue_cancelled');
      if(this.items[this.index] && this.items[this.index].state!=='completed')this.items[this.index].state='cancelled';
      this.phase='cancelled'; this.error=null;
    } catch(error) { this.ownedPid=owned; this.phase='error'; this.error='stop_failed'; throw error; }
    return this.getStatus();
  }
  dispose() { ++this.generation; this.clear(this.timer); this.manager.off('session-ended',this.onEnded); }
}
module.exports = QueueManager;
