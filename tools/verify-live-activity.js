// Exercise LootLazy's actual Rust activity worker with the authorized AION 2 session.
const fs=require('fs'),path=require('path'),assert=require('node:assert/strict');
const {spawn,execFileSync}=require('child_process');const {chromium}=require(process.env.QUESTRA_PLAYWRIGHT||'playwright');
const root=path.resolve(__dirname,'..'),fixture=fs.mkdtempSync(path.join(root,'data/.test-temp/lootlazy-live-ui-'));
fs.mkdirSync(path.join(fixture,'data'),{recursive:true});
const game=JSON.parse(fs.readFileSync('E:/Programs/LootLazy/data/detectable_cache.json','utf8')).find(g=>g.id==='1445982809792708628');assert.ok(game);
fs.writeFileSync(path.join(fixture,'data/detectable_cache.json'),JSON.stringify([game]));fs.writeFileSync(path.join(fixture,'data/catalog_meta.json'),JSON.stringify({lastUpdated:new Date().toISOString()}));
fs.writeFileSync(path.join(fixture,'data/questra_overrides.json'),JSON.stringify({settings:{scan_local_steam:false},custom_executables:{},saved_games:[]}));
const exe=path.join(root,'dist/LootLazy-Tauri/LootLazy.exe'),child=spawn(exe,['--verify-ui'],{cwd:path.dirname(exe),env:{...process.env,LOOTLAZY_DATA_ROOT:fixture},stdio:'pipe',windowsHide:true});
let browser,page;const stderr=[];child.stderr.on('data',b=>stderr.push(b.toString()));
async function until(fn,timeout=20000){const end=Date.now()+timeout;while(Date.now()<end){try{const value=await fn();if(value)return value;}catch{}await new Promise(r=>setTimeout(r,150));}throw Error('Native activity test timed out');}
(async()=>{try{
 const endpoint=await until(async()=>{const r=await fetch('http://127.0.0.1:9387/json/version');return r.ok&&(await r.json()).webSocketDebuggerUrl;});browser=await chromium.connectOverCDP(endpoint);page=await until(()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url().includes('tauri.localhost')));
 await page.waitForFunction(()=>typeof state!=='undefined'&&state.games.length);await page.evaluate(()=>setLanguage('en'));await page.locator('.game-item').click();await page.locator('#btn-toggle-sim').click();
 await page.waitForFunction(()=>state.session?.appId==='1445982809792708628',{},{timeout:20000});
 const status=await until(async()=>{const data=await page.evaluate(()=>request('/api/status'));const presence=data.simulation.session?.presence;return presence?.state==='ready'||presence?.state==='rejected'?data:null;},30000);
 const session=status.simulation.session;const learned=JSON.parse(fs.readFileSync(path.join(fixture,'data/executable_metadata.json'),'utf8'));
 assert.equal(session.executableName,'Aion2/Binaries/Win64/AION2.exe');assert.equal(learned['1445982809792708628'].executables[0],session.executableName);
 const result={testedAt:new Date().toISOString(),runtime:'LootLazy Rust activity worker',game:session.gameName,appId:session.appId,executable:session.executableName,helperFileCreated:fs.existsSync(session.executablePath),presence:session.presence,visibleDetectionVerified:false};
 fs.writeFileSync(path.join(root,'.internal/AION2_NATIVE_ACTIVITY.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
 await page.evaluate(async()=>{await request('/api/stop',{});window.desktopAPI.close();});await new Promise(resolve=>{if(child.exitCode!==null)return resolve();child.once('exit',resolve);setTimeout(resolve,5000);});
 }finally{
 if(page&&!page.isClosed())await page.evaluate(()=>window.lootlazyRequest('/api/stop',{}).finally(()=>window.desktopAPI.close())).catch(()=>{});
 if(child.exitCode===null){await new Promise(resolve=>{child.once('exit',resolve);setTimeout(resolve,5000);});if(child.exitCode===null){try{execFileSync('taskkill.exe',['/PID',String(child.pid),'/T','/F'],{windowsHide:true,stdio:'ignore'});}catch{}}}
 if(browser)await browser.close().catch(()=>{});
 execFileSync('powershell.exe',['-NoProfile','-ExecutionPolicy','Bypass','-File',path.join(root,'tools/cleanup-tauri-test.ps1'),'-FixtureRoot',fixture],{windowsHide:true,stdio:'pipe'});
 }} )().catch(error=>{console.error(error.stack||error.message);process.exitCode=1;});
