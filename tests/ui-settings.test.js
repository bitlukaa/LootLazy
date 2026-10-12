const test=require('node:test');
const assert=require('node:assert/strict');
const vm=require('node:vm');
const fs=require('node:fs');
const path=require('node:path');
const source=fs.readFileSync(path.join(__dirname,'../public/js/app.js'),'utf8');
function run(name,context){const line=source.split(/\r?\n/).find(line=>line.startsWith('function '+name+'(')||line.startsWith('async function '+name+'('));assert.ok(line,name);return vm.runInNewContext(line+';'+name,context);}
test('failed settings save restores the switch and releases it',async()=>{const control={disabled:false,getAttribute:()=> 'false'};const values=[];const messages=[];const fn=run('setSettingChoice',{document:{querySelector:()=>control},updateToggle:(key,value)=>values.push(value),request:async()=>{throw Error('disk full');},loadSettings:async()=>{},toast:value=>messages.push(value),t:key=>key});await fn('sound_alerts',true);assert.deepEqual(values,[true,false]);assert.equal(control.disabled,false);assert.deepEqual(messages,['settings_error']);});
test('successful settings save reloads persisted values',async()=>{const control={disabled:false,getAttribute:()=> 'false'};let loads=0;const fn=run('setSettingChoice',{document:{querySelector:()=>control},updateToggle:()=>{},request:async()=>({}),loadSettings:async()=>{loads++;},toast:()=>{},t:key=>key});await fn('sound_alerts',true);assert.equal(loads,1);assert.equal(control.disabled,false);});
test('manual refresh respects the 15-minute anti-spam cooldown without hitting the backend',async()=>{const routes=[];const state={};const messages=[];const fn=run('refreshCatalog',{state,$:()=>({}),busy:()=>{},setTimeout:(cb)=>cb(),request:async route=>{routes.push(route);return {changed:false,count:50};},lastRealCatalogFetch:Date.now(),localStorage:{setItem:()=>{}},loadStats:async()=>{},fetchGames:async()=>{},toast:msg=>messages.push(msg),t:key=>key});await fn({},false);assert.deepEqual(routes,[]);assert.deepEqual(messages,['games_current']);assert.equal(state.catalogBusy,false);});
test('session diagnostics show errors without asserting game detection',()=>{const fn=run('activityProblem',{errorText:value=>value});assert.equal(fn({presence:{state:'unavailable'}}),'unavailable');assert.equal(fn({presence:{state:'rejected'}}),'rejected');assert.equal(fn({presence:{state:'ready'}}),null);});

test('catalog refresh sends once at the cooldown boundary and records its time',async()=>{
 let now=900000;const routes=[],saved=[];const context={state:{},$:()=>({}),busy:()=>{},Date:{now:()=>now},setTimeout:cb=>cb(),lastRealCatalogFetch:0,localStorage:{setItem:(...args)=>saved.push(args)},request:async route=>{routes.push(route);return {changed:false,count:50};},loadStats:async()=>{},fetchGames:async()=>{},toast:()=>{},t:key=>key};
 const fn=run('refreshCatalog',context);await fn({},false);assert.deepEqual(routes,['/api/refresh-catalog']);assert.equal(context.lastRealCatalogFetch,now);assert.equal(saved[0][0],'lootlazy_last_catalog_fetch');
 now+=899999;await fn({},false);assert.equal(routes.length,1);now++;await fn({},false);assert.equal(routes.length,2);
});

test('quest refresh applies its one-minute manual cooldown at the boundary',async()=>{
 const start=source.indexOf('async function loadQuests('),end=source.indexOf('async function playQuest(',start);assert.ok(start>=0&&end>start);
 let now=1000000;const routes=[],messages=[],saved=[];const context={state:{quests:[]},questsBusy:false,lastRealQuestFetch:940001,lastQuestRefresh:0,$:()=>({}),busy:()=>{},Date:{now:()=>now,parse:Date.parse},setTimeout:cb=>cb(),renderQuests:()=>{},trackQuestUpdates:()=>{},questMatches:()=>false,localStorage:{setItem:(...args)=>saved.push(args)},toast:value=>messages.push(value),number:String,t:key=>key,request:async route=>{routes.push(route);return {quests:[],last_synced:new Date(now).toISOString()};}};
 const fn=vm.runInNewContext(source.slice(start,end)+';loadQuests',context);await fn(true,false);assert.equal(routes.length,0);assert.deepEqual(messages,['quests_refreshed']);assert.equal(context.questsBusy,false);
 now++;await fn(true,false);assert.deepEqual(routes,['/api/quests?refresh=true']);assert.equal(context.lastRealQuestFetch,now);assert.equal(saved[0][0],'lootlazy_last_quest_fetch');
 await fn(true,false);assert.equal(routes.length,1);
});
test('quest refresh toasts quests_current when existing quests are unchanged',async()=>{
 const start=source.indexOf('async function loadQuests('),end=source.indexOf('async function playQuest(',start);assert.ok(start>=0&&end>start);
 const messages=[];const context={state:{quests:[{id:'100',kind:'play',durationSeconds:900}]},questsBusy:false,lastRealQuestFetch:0,lastQuestRefresh:0,$:()=>({}),busy:()=>{},Date:{now:()=>2000000,parse:Date.parse},setTimeout:cb=>cb(),renderQuests:()=>{},trackQuestUpdates:()=>{},questMatches:()=>false,localStorage:{setItem:()=>{}},toast:value=>messages.push(value),number:String,t:key=>key,request:async()=>{return {quests:[{id:'100',kind:'play',durationSeconds:900}],last_synced:new Date().toISOString()};}};
 const fn=vm.runInNewContext(source.slice(start,end)+';loadQuests',context);
 await fn(true,false);
 assert.deepEqual(messages,['quests_current']);
});
test('quest refresh toasts quests_new_found when new quests arrive',async()=>{
 const start=source.indexOf('async function loadQuests('),end=source.indexOf('async function playQuest(',start);assert.ok(start>=0&&end>start);
 const messages=[];const context={state:{quests:[{id:'100',kind:'play',durationSeconds:900}]},questsBusy:false,lastRealQuestFetch:0,lastQuestRefresh:0,$:()=>({}),busy:()=>{},Date:{now:()=>2000000,parse:Date.parse},setTimeout:cb=>cb(),renderQuests:()=>{},trackQuestUpdates:()=>{},questMatches:()=>false,localStorage:{setItem:()=>{}},toast:value=>messages.push(value),number:String,t:key=>key,request:async()=>{return {quests:[{id:'100',kind:'play',durationSeconds:900},{id:'200',kind:'play',durationSeconds:900}],last_synced:new Date().toISOString()};}};
 const fn=vm.runInNewContext(source.slice(start,end)+';loadQuests',context);
 await fn(true,false);
 assert.deepEqual(messages,['quests_new_found']);
});
