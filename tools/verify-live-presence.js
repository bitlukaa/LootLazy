const fs=require('fs'),path=require('path'),os=require('os'),assert=require('node:assert/strict');
const manager=require('../src/processManager'),resolver=require('../src/resolver');
(async()=>{const dir=fs.mkdtempSync(path.join(os.tmpdir(),'questro-live-presence-'));manager.gamesRoot=path.join(dir,'games');manager.historyPath=path.join(dir,'history.json');try{
 const games=JSON.parse(fs.readFileSync(path.resolve('dist/Questro/resources/app/data/detectable_cache.json'),'utf8')),game=games.find(g=>g.name==='Counter-Strike 2');assert.ok(game);const executable=resolver.resolve(game).primaryExecutable;assert.ok(executable);
 const cases=[];const quest=new (require('../src/questFeed').QuestFeed)(path.resolve('dist/Questro/resources/app/data/active_quests.json')).get().quests.find(q=>q.gameName.toLowerCase().includes('deadzone'));
 const scenarios=[{game,executable},...(quest?[{game:{id:quest.appId,name:quest.gameName},executable:null}]:[])];
 for(const scenario of scenarios){await manager.startSimulation(scenario.game.id,scenario.game.name,scenario.executable,1,scenario.game.icon_hash);
 for(let attempt=0;attempt<40&&manager.presence.status().state==='connecting';attempt++)await new Promise(r=>setTimeout(r,250));
 const presence=manager.presence.status();cases.push({game:scenario.game.name,appId:scenario.game.id,executable:scenario.executable,mode:manager.getStatus().session.mode,helperProcessStarted:!!manager.child,presence,discordActivityAcknowledged:presence.activityConfirmed});await manager.stopSimulation();}
 const result={testedAt:new Date().toISOString(),cases,allActivitiesAcknowledged:cases.every(c=>c.discordActivityAcknowledged),questProgressVerified:false};fs.writeFileSync('docs/LIVE_DISCORD_PRESENCE.json',JSON.stringify(result,null,2));console.log(JSON.stringify(result,null,2));
 }finally{await manager.stopSimulation();assert.ok(path.resolve(dir).startsWith(path.resolve(os.tmpdir(),'questro-live-presence-')));fs.rmSync(dir,{recursive:true,force:true});}})().catch(e=>{console.error(e.message);process.exitCode=1});
