const fs=require('fs'),path=require('path'),http=require('http'),assert=require('assert/strict');
const {chromium}=require(process.env.LOOTLAZY_PLAYWRIGHT||'playwright');
const root=path.resolve(__dirname,'..'),publicRoot=path.join(root,'public');
let server,browser;
(async()=>{try{
 server=http.createServer((request,response)=>{const name=new URL(request.url,'http://localhost').pathname;const file=path.resolve(publicRoot,'.'+(name==='/'?'/index.html':name));if(!file.startsWith(publicRoot+path.sep)){response.writeHead(403);response.end();return;}fs.readFile(file,(error,buffer)=>{if(error){response.writeHead(404);response.end();return;}response.setHeader('Content-Type',file.endsWith('.js')?'text/javascript':file.endsWith('.css')?'text/css':file.endsWith('.svg')?'image/svg+xml':'text/html');response.end(buffer);});});await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
 browser=await chromium.launch({...(process.env.LOOTLAZY_BROWSER?{executablePath:process.env.LOOTLAZY_BROWSER}:{}),headless:true});const page=await browser.newPage({viewport:{width:1100,height:750}});const errors=[];page.on('pageerror',error=>errors.push(error.message));
 await page.route('http://artwork.localhost/**',route=>route.fulfill({status:404,body:''}));
 await page.addInitScript(()=>{
  window.lootlazyDiagnostic=true;window.lootlazyObservedRoutes=[];window.lootlazyIntervals=[];const nativeInterval=window.setInterval;window.setInterval=(callback,delay,...args)=>{window.lootlazyIntervals.push(delay);return nativeInterval(callback,delay,...args);};Object.defineProperty(navigator,'clipboard',{value:{writeText:async value=>{window.cryptoCopiedValue=value;}}});const store={variety:{items:[],index:0,phase:'idle',active:false},steam:{items:[],index:0,phase:'idle',active:false}};let generation=0,pool=[];
  const game=n=>({id:String(100000+n),name:'Game '+n,type:1,resolution:{executables:[],primaryExecutable:null}});
  window.lootlazyRequest=async(route,body)=>{const url=new URL(route,'http://localhost'),p=url.pathname;window.lootlazyObservedRoutes.push(p);
   if(p.startsWith('/api/steam-accounts/')){
     const accounts=window.accountFixture ||= [];const command=p.split('/').at(-1);

     if(command==='add'){window.accountAddContainsPassword=Object.hasOwn(body,'password');}if(command==='add')accounts.push({id:'ll_'+String(accounts.length+1).padStart(32,'0'),label:body.label,login:body.login,games:[],state:'offline',active:false,connected:false});
     const account=body?.id?accounts.find(item=>item.id===body.id):null;
     if(command==='library')return {games:account.login==='main_login'?[{id:570,name:'Dota 2'},{id:730,name:'Counter-Strike 2'}]:[{id:440,name:'Team Fortress 2'}],total:2,loading:false,active:true};
     if(command==='games')account.games=body.games;
     if(command==='signin'){account.state='input';account.active=true;account.input={id:'challenge-'+account.id,type:'SteamGuard'};window.accountPasswordWasSubmitted=!!body.password;}
     if(command==='input'){if(body.value!=='12345')throw Error('accounts_input_expired');account.state='ready';account.connected=true;account.input=null;}
     if(command==='start'){account.state='running';account.elapsedSeconds=42;account.minutes=body.minutes;}
     if(command==='stop'){account.state='offline';account.active=false;account.connected=false;account.input=null;}
     if(command==='remove')accounts.splice(accounts.indexOf(account),1);
     return structuredClone({protocol:1,accounts,active:accounts.some(item=>item.active)});
    }
    if(p==='/api/games')return {total:600,games:Array.from({length:50},(_,n)=>game(n)),hasMore:true};
   if(p==='/api/catalog-stats')return {totalGames:600};

   if(p==='/api/settings')return {desktop_notifications:false,total_playtime_seconds:0};
   if(p==='/api/quests')return {quests:window.lootlazyFreezeQuestStatus?state.quests:[],last_synced:new Date().toISOString()};
   if(p==='/api/updates/install'){window.updateInstallCalled=true;throw Error('updates_stop_sessions');}
   if(p.startsWith('/api/updates'))return {currentVersion:'3.4.0',status:'restricted',currentNotes:{en:['Game Variety and Idle Steam'],ar:[]}};
   if(p==='/api/platforms/status')return {steamInstalled:window.lootlazyFixtureSteamInstalled??true,discordInstalled:window.lootlazyFixtureDiscordInstalled??true};if(p==='/api/platforms/download'){window.platformDownload=body.platform;return {success:true};}if(p==='/api/status')return {discordConnected:false,steamRunning:window.lootlazyFixtureSteamRunning??true,steamConnected:store.steam.items.some(item=>item.state==='running'),simulation:window.lootlazyFreezeQuestStatus?{active:!!state.session,session:state.session}:{active:false},queue:{items:[],active:false,phase:'idle',index:-1},questQueue:window.lootlazyFreezeQuestStatus?state.questQueue:{items:[],active:false,phase:'idle'},variety:store.variety,steamIdle:{rotation:store.steam,runtimeReady:true}};
   if(p==='/api/variety')return structuredClone(store.variety);
   if(p==='/api/variety/start'){store.variety.active=true;store.variety.phase='running';store.variety.items[store.variety.index].state='running';store.variety.remainingSeconds=42;return structuredClone(store.variety);}
   if(p==='/api/variety/pause'){store.variety.active=false;store.variety.phase='paused';store.variety.items[store.variety.index].state='paused';return structuredClone(store.variety);}
   if(p==='/api/variety/stop'){store.variety.active=false;store.variety.phase='idle';for(const item of store.variety.items)item.state='queued';return structuredClone(store.variety);}
   if(p==='/api/variety/shuffle'){generation+=600;pool=Array.from({length:500},(_,n)=>({id:String(generation+n),appId:String(generation+n),gameName:'Game '+(n+1),durationMinutes:body.durationMinutes,state:'queued',elapsedSeconds:0,icon_hash:'0123456789abcdef0123456789abcdef'}));store.variety={items:pool.slice(0,body.count),index:0,phase:'idle',active:false};return structuredClone(store.variety);}
   if(p==='/api/variety/resize'){store.variety.items=pool.slice(0,body.count);return structuredClone(store.variety);}
   if(p==='/api/variety/duration'){for(const item of pool)item.durationMinutes=body.durationMinutes;store.variety.defaultMinutes=body.durationMinutes;return structuredClone(store.variety);}
   if(p==='/api/variety/items'){const item=store.variety.items.find(i=>i.id===body.id);if(body.action==='duration')item.durationMinutes=body.durationMinutes;if(['up','down'].includes(body.action)){const index=store.variety.items.indexOf(item),next=index+(body.action==='up'?-1:1);[store.variety.items[index],store.variety.items[next]]=[store.variety.items[next],store.variety.items[index]];}return structuredClone(store.variety);}
   if(p==='/api/steam/status')return {rotation:store.steam,runtimeReady:true};
   if(p==='/api/steam/games')return {games:[{id:'730',steamId:'730',discordAppId:'100000',name:'Counter-Strike 2',third_party_skus:[{distributor:'steam',id:'730'}]},{id:'431960',steamId:'431960',name:'Wallpaper Engine',third_party_skus:[{distributor:'steam',id:'431960'}]}],total:2,runtimeReady:true};
   if(p==='/api/steam/start'||p==='/api/steam/sync'){window.lootlazyLastSteamAction=p;const selected=body.sessions||[{steamId:body.steamId,durationMinutes:body.durationMinutes}];for(const choice of selected){let item=store.steam.items.find(i=>i.steamId===choice.steamId);if(!item){item={id:choice.steamId,appId:choice.steamId,steamId:choice.steamId,gameName:choice.steamId==='730'?'Counter-Strike 2':'Wallpaper Engine'};store.steam.items.push(item);}Object.assign(item,{durationMinutes:choice.durationMinutes,state:'running',elapsedSeconds:0,remainingSeconds:choice.durationMinutes?choice.durationMinutes*60:null});}store.steam.active=true;return structuredClone(store.steam);}
   if(p==='/api/steam/stop'){store.steam.items=store.steam.items.filter(i=>body.steamId&&i.steamId!==body.steamId);store.steam.active=store.steam.items.length>0;return structuredClone(store.steam);}
   throw Error('Unexpected fixture route: '+p);
  };
 });
 await page.goto('http://127.0.0.1:'+server.address().port);await page.waitForFunction(()=>state.games.length===50);
 assert.ok(!(await page.evaluate(()=>window.lootlazyObservedRoutes)).some(route=>route.startsWith('/api/steam-accounts/')),'Account engine remains lazy');
 await page.click('[data-tab=steam]');
 assert.deepEqual(await page.locator('[data-steam-sub-mode]').evaluateAll(items=>items.map(item=>item.dataset.steamSubMode)),['library','accounts','active']);
 await page.click('[data-steam-sub-mode=accounts]');await page.waitForFunction(()=>!document.getElementById('accounts-add').disabled);assert.equal(await page.locator('#accounts-empty').isVisible(),true);
 async function add(label,login){await page.click('#accounts-add');await page.fill('#account-new-label',label);await page.fill('#account-new-login',login);await page.fill('#account-new-password','test-password-not-persisted');await page.click('#account-add-form button[type=submit]');await page.waitForFunction(()=>!document.getElementById('accounts-add').disabled);}
 await add('Main','main_login');assert.equal(await page.locator('#account-title').innerText(),'Main');assert.equal(await page.locator('#account-duration').inputValue(),'0');
 await page.waitForFunction(()=>document.getElementById('account-verify-dialog').open);assert.equal(await page.locator('#account-new-password').inputValue(),'');assert.equal(await page.evaluate(()=>window.accountAddContainsPassword),false);assert.equal(await page.evaluate(()=>window.accountPasswordWasSubmitted),true);assert.equal(await page.locator('#account-setup input[type=password]').count(),0);assert.equal(await page.locator('[data-steam-sub-mode=accounts] .feature-new-badge').isVisible(),true);assert.equal(await page.locator('#account-input-label').innerText(),'Code from Steam email');
 await page.fill('#account-code','wrong');await page.click('#account-submit-input');await page.waitForFunction(()=>!document.getElementById('accounts-error').hidden);assert.match(await page.locator('#accounts-error').innerText(),/expired/);assert.equal(await page.locator('#account-code').inputValue(),'');
 await page.fill('#account-code','12345');await page.click('#account-submit-input');await page.waitForFunction(()=>document.getElementById('account-state').textContent==='Ready');
 assert.equal(await page.locator('[data-steam-sub-mode=accounts] .feature-new-badge').isVisible(),false);await page.waitForFunction(()=>document.querySelectorAll('#account-quick-picks button').length===2);assert.match(await page.locator('#account-quick-picks').innerText(),/Counter-Strike 2/);
 await page.fill('#account-search','Dota');await page.locator('#account-search-results button').first().click();await page.waitForFunction(()=>document.querySelectorAll('#account-games .account-game-row').length===1);
 assert.match(await page.locator('#account-games').innerText(),/Dota 2/);
 await page.click('#account-start');await page.waitForFunction(()=>document.getElementById('account-state').textContent==='Running');assert.equal(await page.locator('#account-start').isVisible(),false);assert.equal(await page.locator('#account-running-time').innerText(),'00:00:42');
 await add('Second','second_login');await page.click('#account-verify-close');assert.equal(await page.locator('#accounts-list .account-row').count(),2);assert.equal(await page.locator('#account-title').innerText(),'Second');assert.equal(await page.locator('#account-games .account-game-row').count(),0,'Games are isolated between accounts');
 await page.locator('#accounts-list .account-row').first().click();assert.match(await page.locator('#account-games').innerText(),/Dota 2/);assert.equal(await page.locator('#account-password').inputValue(),'');
 await page.screenshot({path:path.join(root,'data/steam-accounts-ui.png')});
 await page.click('[data-steam-sub-mode=active]');assert.equal(await page.locator('#accounts-active-list .account-game-row').count(),1);assert.match(await page.locator('#accounts-active-list').innerText(),/Main/);await page.locator('#accounts-active-list button').click();await page.waitForFunction(()=>document.getElementById('accounts-active-panel').hidden);
 await page.click('[data-steam-sub-mode=accounts]');await page.evaluate(()=>setLanguage('ar'));assert.equal(await page.locator('html').getAttribute('dir'),'rtl');assert.equal(await page.locator('#account-signin-open').innerText(),await page.evaluate(()=>t('accounts_signin')));
 await page.click('#account-remove');await page.click('#account-remove-cancel');assert.equal(await page.locator('#accounts-list .account-row').count(),2);await page.click('#account-remove');await page.click('#account-remove-confirm');await page.waitForFunction(()=>document.querySelectorAll('#accounts-list .account-row').length===1);
 assert.equal(await page.evaluate(()=>JSON.parse(localStorage.getItem('lootlazy_used_features')).includes('steam-accounts-v1')),true);
 assert.equal(await page.locator('#account-title').innerText(),'Second');assert.equal(await page.locator('#steam-accounts-mode-view img').count(),0,'No game images in account management');assert.deepEqual(errors,[]);
 await page.reload();await page.waitForFunction(()=>state.games.length===50);assert.equal(await page.locator('[data-tab=steam] .feature-new-badge').isVisible(),false,'New stays hidden after a successful sign-in and reload');
 console.log('Steam Accounts renderer: tab placement, lazy start, account isolation, search, Guard retry, secret clearing, running controls, active sessions, removal and Arabic passed.');
 }finally{if(browser)await browser.close();if(server)await new Promise(resolve=>server.close(resolve));}})().catch(error=>{console.error(error);process.exitCode=1;});
