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


test('manual up-to-date checks produce a toast while silent checks do not',async()=>{
 for(const silent of [false,true]){const messages=[];const fn=run('loadUpdates',{updatesBusy:false,state:{update:{latestVersion:'3.6.15'},tab:'settings'},$:()=>({}),busy:()=>{},request:async()=>({status:'up_to_date',latestVersion:'3.6.15'}),renderUpdates:()=>{},toast:message=>messages.push(message),t:key=>key,window:{lootlazyDiagnostic:true}});await fn(true,silent);assert.deepEqual(messages,silent?[]:['updates_current']);}
});


test('catalog refresh retries one transient transport failure without an error toast',async()=>{
 let calls=0;const messages=[];const context={number:value=>String(value),state:{games:[]},$:()=>({}),busy:()=>{},Date:{now:()=>900000},setTimeout:cb=>cb(),lastRealCatalogFetch:0,localStorage:{setItem:()=>{}},request:async()=>{calls++;if(calls===1)throw Error('network_unavailable');return {changed:false,count:50};},loadStats:async()=>{},fetchGames:async()=>{},toast:(message,type)=>messages.push([message,type]),t:key=>key};
 await run('refreshCatalog',context)({},false);assert.equal(calls,2);assert.deepEqual(messages,[['games_current','success']]);assert.equal(context.state.catalogBusy,false);assert.equal(context.lastRealCatalogFetch,900000);
});
test('catalog refresh does not retry validation failures or hide persistent connection errors',async()=>{
 for(const error of ['Invalid catalog metadata','network_unavailable']){let calls=0;const messages=[];const context={state:{games:[{id:'1'}]},$:()=>({}),busy:()=>{},Date:{now:()=>900000},setTimeout:cb=>cb(),lastRealCatalogFetch:0,localStorage:{setItem:()=>{}},request:async()=>{calls++;throw Error(error);},loadStats:async()=>{},fetchGames:async()=>{},toast:(message,type)=>messages.push([message,type]),t:key=>key};await run('refreshCatalog',context)({},false);assert.equal(calls,error==='network_unavailable'?2:1);assert.deepEqual(messages,[['catalog_error','error']]);assert.equal(context.state.games.length,1);assert.equal(context.lastRealCatalogFetch,0);}
});
test('update discovery never writes to the session notification history',async()=>{
 for(const check of [true,false]){const history=[];const context={updatesBusy:false,state:{update:{},tab:'settings'},$:()=>({}),busy:()=>{},request:async()=>({status:'available',latestVersion:'3.6.16'}),renderUpdates:()=>{},toast:()=>{},t:key=>key,window:{lootlazyDiagnostic:true,lootlazyNotifications:{add:(...args)=>history.push(args)}}};await run('loadUpdates',context)(check,false);assert.equal(history.length,0);}
});
test('unchanged action labels preserve button contents between status polls',()=>{let changes=0;let current='';const button={dataset:{},firstElementChild:{},get innerHTML(){return current},set innerHTML(value){changes++;current=value;}};const fn=run('actionLabel',{getIcon:icon=>'<svg>'+icon+'</svg>',escapeHtml:value=>String(value)});fn(button,'play','Start Quest');fn(button,'play','Start Quest');assert.equal(changes,1);fn(button,'play','Resume');assert.equal(changes,2);button.firstElementChild=null;fn(button,'play','Resume');assert.equal(changes,3,'label is restored after another state removes the icon');});
test('failed initial quest loading shows an error instead of implying an empty successful feed',async()=>{
 const start=source.indexOf('async function loadQuests('),end=source.indexOf('async function playQuest(',start);const controls=new Map();const control=id=>{if(!controls.has(id))controls.set(id,{hidden:true,textContent:''});return controls.get(id);};
 const context={questsBusy:false,lastRealQuestFetch:0,lastQuestRefresh:0,state:{quests:[],session:null},$:control,busy:()=>{},Date,renderQuests:()=>{},request:async()=>{throw Error('network_unavailable');},t:key=>key};
 const fn=vm.runInNewContext(source.slice(start,end)+';loadQuests',context);await fn(false,true);assert.equal(control('quests-error').hidden,false);assert.equal(control('quests-error').textContent,'quests_load_error');assert.equal(context.questsBusy,false);
});
test('update installation explains active sessions for native string and Error rejections',async()=>{
 for(const rejected of ['updates_stop_sessions',new Error('updates_stop_sessions')]){const messages=[],button={disabled:false,textContent:''};const context={state:{},$:()=>button,t:key=>key,request:async()=>{throw rejected},toast:(message,type)=>messages.push([message,type])};await run('installUpdate',context)();assert.deepEqual(messages,[['updates_stop_sessions','error']]);assert.equal(context.state.installingUpdate,false);assert.equal(button.disabled,false);assert.equal(button.textContent,'install_update');}
});
test('other native update failures preserve their reason and unknown failures stay generic',async()=>{
 for(const code of ['updates_busy','updates_unavailable','updates_download_failed','updates_checksum_failed','updates_signature_failed','updates_save_failed','updates_install_failed','unknown_error']){const messages=[],button={};const fn=run('installUpdate',{state:{},$:()=>button,t:key=>key,request:async()=>{throw code},toast:message=>messages.push(message)});await fn();assert.deepEqual(messages,[code==='unknown_error'?'updates_install_failed':code]);assert.equal(button.disabled,false);}
});

test('update discovery cannot race an active download or restart',async()=>{let calls=0;const fn=run('loadUpdates',{updatesBusy:false,state:{installingUpdate:true},request:async()=>{calls++;},$:()=>{throw Error('No UI change during installation');}});await fn(true);assert.equal(calls,0);});

test('a pending update check cannot replace installer state with a busy error',async()=>{const state={update:{status:'available'}};let renders=0;const fn=run('loadUpdates',{updatesBusy:false,state,$:()=>({}),busy:()=>{},request:async()=>{state.installingUpdate=true;throw Error('updates_busy');},renderUpdates:()=>{renders++;},window:{lootlazyDiagnostic:true}});await fn(true);assert.equal(state.update.status,'available');assert.equal(renders,0);});

test('native errors retain their actionable domain code through diagnostic wrappers',()=>{const fn=run('nativeErrorCode',{I18N:{en:{operation_failed:'Generic',achievements_not_supported:'Runtime',accounts_credentials_invalid:'Credentials'}}});assert.equal(fn('operation_failed: achievements_not_supported: SteamAPI_Init',['achievements_']),'achievements_not_supported');assert.equal(fn(Error('operation_failed: accounts_credentials_invalid'),['accounts_']),'accounts_credentials_invalid');assert.equal(fn('unrecognized private path',['accounts_']),'');});
