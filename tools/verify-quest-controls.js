const fs = require('fs');
const path = require('path');
const http = require('http');
const { chromium } = require(process.env.LOOTLAZY_PLAYWRIGHT || 'playwright');

const root = path.resolve(__dirname, '..');
const publicRoot = path.join(root, 'public');
const screenshotsDir = path.join(root, 'docs/screenshots');

const popularGames=[{id:'123456',name:'Example Game',resolution:{executables:[],primaryExecutable:null}}];
const mockQuests = JSON.parse(fs.readFileSync(path.join(screenshotsDir, 'quest-examples.json'), 'utf8').replace(/^\uFEFF/, ''));

let server, browser;

(async () => {
  try {
    server = http.createServer((request, response) => {
      const name = new URL(request.url, 'http://localhost').pathname;
      const file = path.resolve(publicRoot, '.' + (name === '/' ? '/index.html' : name));
      if (!file.startsWith(publicRoot + path.sep)) {
        response.writeHead(403);
        response.end();
        return;
      }
      fs.readFile(file, (error, buffer) => {
        if (error) {
          response.writeHead(404);
          response.end();
          return;
        }
        response.setHeader('Content-Type', file.endsWith('.js') ? 'text/javascript' : file.endsWith('.css') ? 'text/css' : file.endsWith('.svg') ? 'image/svg+xml' : 'text/html');
        response.end(buffer);
      });
    });

    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));

    browser = await chromium.launch({
      ...(process.env.LOOTLAZY_BROWSER ? { executablePath: process.env.LOOTLAZY_BROWSER } : {}),
      headless: true
    });

    const page = await browser.newPage({
      viewport: { width: 1136, height: 916 },
      deviceScaleFactor: 1
    });

    await page.route('http://artwork.localhost/**',route=>route.fulfill({status:404,body:''}));

    await page.addInitScript(({ games, quests }) => {
      window.lootlazyDiagnostic = true;
      const store = {
        variety: {
          items: games.slice(0, 10).map((g, i) => ({
            id: g.id,
            appId: g.id,
            icon_hash:g.icon_hash,
            cover_image_hash:g.cover_image_hash,
            gameName: g.name,
            durationMinutes: 15,
            state: 'queued',
            elapsedSeconds: 0
          })),
          index: 0,
          phase: 'idle',
          active: false,
          defaultMinutes: 15
        },
        steam: {
          items: [],
          index: 0,
          phase: 'idle',
          active: false
        }
      };

      window.lootlazyScreenshotState=store;
      window.lootlazyRequest=async(route,body)=>{if(route==='/api/identity/status')return {enforced:false,configured:false,authenticated:false};
        const url = new URL(route, 'http://localhost');
        const p = url.pathname;
        if (p === '/api/games') return { total: games.length, games, hasMore: false };
        if (p === '/api/catalog-stats') return { totalGames: games.length };
        if (p === '/api/settings') return { desktop_notifications: false, total_playtime_seconds: 0 };
        if (p === '/api/quests') return { quests, last_synced: new Date().toISOString() };
        if (p.startsWith('/api/updates')) return { currentVersion: '3.6.18', status: 'up_to_date' };
        if (p === '/api/status') return {
          discordConnected: true,
          steamRunning: true,
          steamConnected: true,
          simulation: { active: false },
          queue: { items: [], active: false, phase: 'idle', index: -1 },
          questQueue: { items: [], active: false, phase: 'idle' },
          variety: store.variety,
          steamIdle: { rotation: store.steam, runtimeReady: true }
        };
        if (p === '/api/variety') return structuredClone(store.variety);
        if (p === '/api/steam/status') return { rotation: store.steam, runtimeReady: true };
        if (p === '/api/steam/games') return {
          games: [
            { id: '730', steamId: '730', discordAppId: '730', name: 'Counter-Strike 2', third_party_skus: [{ distributor: 'steam', id: '730' }] },
            { id: '570', steamId: '570', discordAppId: '570', name: 'Dota 2', third_party_skus: [{ distributor: 'steam', id: '570' }] },
            { id: '1172470', steamId: '1172470', discordAppId: '1172470', name: 'Apex Legends', third_party_skus: [{ distributor: 'steam', id: '1172470' }] },
            { id: '431960', steamId: '431960', discordAppId: '431960', name: 'Wallpaper Engine', third_party_skus: [{ distributor: 'steam', id: '431960' }] },
            { id: '252490', steamId: '252490', discordAppId: '252490', name: 'Rust', third_party_skus: [{ distributor: 'steam', id: '252490' }] }
          ],
          total: 5,
          runtimeReady: true
        };
        return {};
      };
    }, { games: popularGames, quests: mockQuests });

    await page.goto('http://127.0.0.1:' + server.address().port);
    await page.waitForFunction(() => typeof state !== 'undefined' && state.games && state.games.length > 0);
    await page.evaluate(() => {
      setLanguage('en');
      applyTheme('light');
      updateDiscordIndicator(true);
      updateSteamIndicator(true, false);
    });

    const settle = async () => {
      await page.evaluate(async () => {
        await Promise.all(document.getAnimations().filter(a => a.effect?.getComputedTiming().iterations !== Infinity).map(a => a.finished.catch(() => {})));
      });
      await page.waitForTimeout(300);
    };


    const failures=[];
    await page.evaluate(()=>{switchTab('quests');state.quests[1].localFinished=true;renderQuests();});
    await settle();
    for(const lang of ['en','ar'])for(const theme of ['light','dark']){
      await page.evaluate(({lang,theme})=>{setLanguage(lang);applyTheme(theme);}, {lang,theme});await settle();
      const start=page.locator('.quest-play:not(:disabled)').first();
      const before=await start.boundingBox();await start.hover();await page.mouse.down();await page.waitForTimeout(80);const pressed=await start.boundingBox();await page.mouse.move(0,0);await page.mouse.up();
      if(Math.abs(before.y-pressed.y)>.1||Math.abs(before.x-pressed.x)>.1)failures.push(lang+'/'+theme+': pressed Start Quest moves');
      const stable=await page.evaluate(()=>{const closed=document.querySelector('.quest-closed .quest-play');const original=closed.firstChild;const rect=closed.getBoundingClientRect();for(let i=0;i<10;i++)updateQuestSessions();return {sameNode:original===closed.firstChild,y:closed.getBoundingClientRect().y-rect.y};});
      if(!stable.sameNode)failures.push(lang+'/'+theme+': Closed contents replaced on every status tick');
      if(Math.abs(stable.y)>.1)failures.push(lang+'/'+theme+': Closed geometry changes');
    }
    const activeChecks=await page.evaluate(()=>{state.session={questId:state.quests[0].id,remainingFormatted:'00:14:59',progressPercent:1};updateQuestSessions();const card=document.querySelector('[data-quest-id="'+state.session.questId+'"]');const pauseHasIcon=!!card.querySelector('.quest-pause svg');state.session.remainingFormatted='00:14:58';updateQuestSessions();return {pauseHasIcon,timerUpdated:card.querySelector('.quest-play').textContent==='00:14:58'};});
    if(!activeChecks.pauseHasIcon)failures.push('Active quest loses its Pause icon during polling');
    if(!activeChecks.timerUpdated)failures.push('Active quest countdown stopped updating');
    if(failures.length)throw Error(failures.join('\n'));
    console.log('Quest controls remain still during press and repeated status ticks in English/Arabic, light/dark.');
  } finally {
    if (browser) await browser.close();
    if (server) await new Promise(resolve => server.close(resolve));
  }
})().catch(err => {
  console.error(err);
  process.exitCode = 1;
});
