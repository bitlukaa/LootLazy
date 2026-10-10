const fs=require('fs');const path=require('path');
const SOURCE='https://api.discordquest.com/api/quests';
function assetUrl(value,questId){
 if(typeof value!=='string'||!value||value==='PLACEHOLDER')return null;
 const candidate=value.startsWith('https://')?value:value.replace(/^\//,'').startsWith('quests/')?'https://cdn.discordapp.com/'+value.replace(/^\//,''):'https://cdn.discordapp.com/quests/'+questId+'/'+value;
 try{const url=new URL(candidate);if(url.protocol==='https:'&&url.hostname==='cdn.discordapp.com'&&/^\/quests\/\d+\/[^?]+\.(png|jpe?g|webp)$/i.test(url.pathname)&&!url.search&&!url.hash)return url.href;}catch{}return null;
}
function normalizeQuests(payload,now=Date.now()){
 const list=Array.isArray(payload)?payload:payload?.quests;
 if(!Array.isArray(list))throw new Error('Invalid quest feed');
 const seen=new Set(),results=[];
 for(const raw of list){
  const c=raw?.config||raw,id=String(raw?.id||c?.id||'');if(!/^\d{5,22}$/.test(id)||seen.has(id))continue;
  const starts=Date.parse(c.starts_at),ends=Date.parse(c.expires_at);
  if(!Number.isFinite(starts)||!Number.isFinite(ends)||starts>now||ends<=now||ends-starts>366*86400000)continue;
  const tasks=c.task_config_v2?.tasks||c.task_config?.tasks||{};
  const kindKey=['PLAY_ON_DESKTOP','STREAM_ON_DESKTOP','WATCH_VIDEO','WATCH_VIDEO_ON_MOBILE','ACHIEVEMENT_IN_ACTIVITY','PLAY_ACTIVITY'].find(key=>tasks[key]);
  if(kindKey!=='PLAY_ON_DESKTOP'||Object.keys(tasks).some(key=>/WATCH|ACTIVITY/.test(key)))continue;
  const task=tasks[kindKey],target=Number(task.target);if(target!==900)continue;
  const appId=String(task.applications?.[0]?.id||c.application?.id||'');
  const kind=kindKey==='PLAY_ON_DESKTOP'?'play':kindKey==='STREAM_ON_DESKTOP'?'stream':kindKey.startsWith('WATCH_')?'video':'activity';
  const name=String(c.messages?.game_title||c.application?.name||c.messages?.quest_name||'Game').slice(0,200);
  const title=String(c.messages?.quest_name||name).slice(0,250);
  if(/placeholder|test quest|quest test/i.test(title))continue;
  const assets=c.assets||{},image=[assets.hero,assets.game_tile_dark,assets.game_tile_light,assets.game_tile].map(value=>assetUrl(value,id)).find(Boolean)||null;
  const rewards=(c.rewards_config?.rewards||[]).map(reward=>({name:String(reward.messages?.name||'').slice(0,150),type:reward.type,quantity:Number(reward.orb_quantity||reward.quantity||0)}));
  seen.add(id);results.push({id,rewards,appId:/^\d{5,22}$/.test(appId)?appId:null,gameName:name,questName:title,kind,durationSeconds:kind==='activity'?null:target,durationMinutes:kind==='play'||kind==='stream'?Math.ceil(target/60):null,target:kind==='activity'?target:null,startsAt:new Date(starts).toISOString(),expiresAt:new Date(ends).toISOString(),image});
 }
 const campaigns=new Map();
 for(const quest of results){const key=JSON.stringify([quest.questName,quest.appId,quest.kind,quest.durationSeconds,quest.target,quest.startsAt,quest.expiresAt,quest.rewards]);const existing=campaigns.get(key);if(existing)existing.variantIds.push(quest.id);else campaigns.set(key,{...quest,variantIds:[quest.id]});}
 return [...campaigns.values()].sort((a,b)=>Date.parse(b.startsAt)-Date.parse(a.startsAt));
}
class QuestFeed{
 closedStates(){try{return JSON.parse(fs.readFileSync(this.file+'.states.json','utf8'));}catch{return {};}}
 closeQuest(id,reason){const states=this.closedStates();states[id]={reason,closedAt:new Date().toISOString()};fs.mkdirSync(path.dirname(this.file),{recursive:true});fs.writeFileSync(this.file+'.states.json.tmp',JSON.stringify(states,null,2));fs.renameSync(this.file+'.states.json.tmp',this.file+'.states.json');}
 reopenQuest(id){this.closeQuest(id,'open');}
 constructor(file=path.resolve(__dirname,'../data/active_quests.json'),fetchSource=fetch){this.file=file;this.fetchSource=fetchSource;this.pending=null;}
 get(){try{const data=JSON.parse(fs.readFileSync(this.file,'utf8'));if(data.source!=='community_feed')return {quests:[],last_synced:null,source:SOURCE};return {...data,quests:(data.quests||[]).filter(q=>q.kind==='play'&&q.durationSeconds===900&&Date.parse(q.expiresAt)>Date.now())};}catch{return {quests:[],last_synced:null,source:SOURCE};}}
 save(data){fs.mkdirSync(path.dirname(this.file),{recursive:true});fs.writeFileSync(this.file+'.tmp',JSON.stringify(data,null,2));fs.renameSync(this.file+'.tmp',this.file);}
 async refresh(){if(this.pending)return this.pending;this.pending=this.fetchAndSave().finally(()=>this.pending=null);return this.pending;}
 async fetchAndSave(){const response=await this.fetchSource(SOURCE,{signal:AbortSignal.timeout(15000),redirect:'error',headers:{Accept:'application/json'}});if(!response.ok)throw new Error('Quest source HTTP '+response.status);let payload;if(response.body){const chunks=[];let size=0;for await(const chunk of response.body){size+=chunk.length;if(size>12*1024*1024)throw new Error('Quest feed too large');chunks.push(chunk);}payload=JSON.parse(Buffer.concat(chunks).toString('utf8'));}else payload=await response.json();const quests=normalizeQuests(payload);const data={source:'community_feed',sourceUrl:SOURCE,last_synced:new Date().toISOString(),quests};this.save(data);return data;}
 clear(){const data={source:'community_feed',sourceUrl:SOURCE,last_synced:null,quests:[]};this.save(data);return data;}
}
module.exports=new QuestFeed();module.exports.QuestFeed=QuestFeed;module.exports.normalizeQuests=normalizeQuests;module.exports.SOURCE=SOURCE;
