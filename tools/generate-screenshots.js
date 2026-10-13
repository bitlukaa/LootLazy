const fs = require('fs');
const path = require('path');
const http = require('http');
const { chromium } = require(process.env.LOOTLAZY_PLAYWRIGHT || 'playwright');

const root = path.resolve(__dirname, '..');
const publicRoot = path.join(root, 'public');
const screenshotsDir = path.join(root, 'docs/screenshots');

const popularGames = [
  { id: '730', name: 'Counter-Strike 2', type: 1, third_party_skus: [{ distributor: 'steam', id: '730' }], resolution: { executables: [{ os: 'win32', name: 'cs2.exe' }], primaryExecutable: 'cs2.exe' } },
  { id: '570', name: 'Dota 2', type: 1, third_party_skus: [{ distributor: 'steam', id: '570' }], resolution: { executables: [{ os: 'win32', name: 'dota2.exe' }], primaryExecutable: 'dota2.exe' } },
  { id: '1172470', name: 'Apex Legends', type: 1, third_party_skus: [{ distributor: 'steam', id: '1172470' }], resolution: { executables: [{ os: 'win32', name: 'r5apex.exe' }], primaryExecutable: 'r5apex.exe' } },
  { id: '271590', name: 'Grand Theft Auto V', type: 1, third_party_skus: [{ distributor: 'steam', id: '271590' }], resolution: { executables: [{ os: 'win32', name: 'GTA5.exe' }], primaryExecutable: 'GTA5.exe' } },
  { id: '431960', name: 'Wallpaper Engine', type: 1, third_party_skus: [{ distributor: 'steam', id: '431960' }], resolution: { executables: [{ os: 'win32', name: 'wallpaper64.exe' }], primaryExecutable: 'wallpaper64.exe' } },
  { id: '1091500', name: 'Cyberpunk 2077', type: 1, third_party_skus: [{ distributor: 'steam', id: '1091500' }], resolution: { executables: [{ os: 'win32', name: 'Cyberpunk2077.exe' }], primaryExecutable: 'Cyberpunk2077.exe' } },
  { id: '252490', name: 'Rust', type: 1, third_party_skus: [{ distributor: 'steam', id: '252490' }], resolution: { executables: [{ os: 'win32', name: 'RustClient.exe' }], primaryExecutable: 'RustClient.exe' } },
  { id: '440', name: 'Team Fortress 2', type: 1, third_party_skus: [{ distributor: 'steam', id: '440' }], resolution: { executables: [{ os: 'win32', name: 'hl2.exe' }], primaryExecutable: 'hl2.exe' } },
  { id: '1086940', name: 'Baldur\'s Gate 3', type: 1, third_party_skus: [{ distributor: 'steam', id: '1086940' }], resolution: { executables: [{ os: 'win32', name: 'bg3_dx11.exe' }], primaryExecutable: 'bg3_dx11.exe' } },
  { id: '105600', name: 'Terraria', type: 1, third_party_skus: [{ distributor: 'steam', id: '105600' }], resolution: { executables: [{ os: 'win32', name: 'Terraria.exe' }], primaryExecutable: 'Terraria.exe' } },
  { id: '359550', name: 'Tom Clancy\'s Rainbow Six Siege', type: 1, third_party_skus: [{ distributor: 'steam', id: '359550' }], resolution: { executables: [{ os: 'win32', name: 'RainbowSix.exe' }], primaryExecutable: 'RainbowSix.exe' } },
  { id: '230410', name: 'Warframe', type: 1, third_party_skus: [{ distributor: 'steam', id: '230410' }], resolution: { executables: [{ os: 'win32', name: 'Warframe.x64.exe' }], primaryExecutable: 'Warframe.x64.exe' } }
];

const mockQuests = [
  {
    id: '123456789012345678',
    questName: 'Example quest · Counter-Strike 2',
    image: 'https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/730/header.jpg',
    gameName: 'Counter-Strike 2',
    game: { id: '730' },
    kind: 'play',
    durationMinutes: 15,
    durationSeconds: 900,
    resumeElapsedSeconds: 0,
    startsAt: new Date(Date.now() - 3600000).toISOString(),
    expiresAt: new Date(Date.now() + 86400000 * 5).toISOString(),
    variantIds: ['730'],
    rewards: [{ name: 'Discord Badge & Avatar Decoration' }]
  },
  {
    id: '234567890123456789',
    questName: 'Example quest · Warframe',
    image: 'https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/230410/header.jpg',
    gameName: 'Warframe',
    game: { id: '230410' },
    kind: 'play',
    durationMinutes: 15,
    durationSeconds: 900,
    resumeElapsedSeconds: 0,
    startsAt: new Date(Date.now() - 7200000).toISOString(),
    expiresAt: new Date(Date.now() + 86400000 * 3).toISOString(),
    variantIds: ['230410'],
    rewards: [{ name: 'In-Game Reward & Discord Badge' }]
  }
];

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

    await page.route('http://artwork.localhost/**', async route => { const url=new URL(route.request().url()).searchParams.get('url'); if(!url||!/^https:\/\/(?:shared\.akamai|shared\.fastly|cdn\.akamai)\.steamstatic\.com\//.test(url))return route.fulfill({status:404,body:''}); try{const response=await route.fetch({url,timeout:15000});await route.fulfill({response});}catch{await route.fulfill({status:404,body:''});} });

    await page.addInitScript(({ games, quests }) => {
      window.lootlazyDiagnostic = true;
      const store = {
        variety: {
          items: games.slice(0, 10).map((g, i) => ({
            id: g.id,
            appId: g.id,
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

      window.lootlazyRequest = async (route, body) => {
        const url = new URL(route, 'http://localhost');
        const p = url.pathname;
        if (p === '/api/games') return { total: games.length, games, hasMore: false };
        if (p === '/api/catalog-stats') return { totalGames: games.length };
        if (p === '/api/settings') return { desktop_notifications: false, total_playtime_seconds: 0 };
        if (p === '/api/quests') return { quests, last_synced: new Date().toISOString() };
        if (p.startsWith('/api/updates')) return { currentVersion: '3.6.11', status: 'up_to_date' };
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

    // 1. Game Time Screenshot
    await page.evaluate(() => {
      switchTab('simulator');
      setSimulatorMode('library');
      selectGame(state.games[0]); // Counter-Strike 2
    });
    await settle();
    const gameTimePath = path.join(screenshotsDir, 'game-time.png');
    await page.screenshot({ path: gameTimePath });
    console.log('Saved:', gameTimePath);

    // 2. Game Variety Screenshot
    await page.evaluate(() => {
      switchTab('variety');
    });
    await settle();
    const gameVarietyPath = path.join(screenshotsDir, 'game-variety.png');
    await page.screenshot({ path: gameVarietyPath });
    console.log('Saved:', gameVarietyPath);

    // 3. Idle Steam Screenshot
    await page.evaluate(() => {
      switchTab('steam');
      setSteamSubMode('library');
      document.querySelector('#steam-library .game-item')?.click();
    });
    await settle();
    const idleSteamPath = path.join(screenshotsDir, 'idle-steam.png');
    await page.screenshot({ path: idleSteamPath });
    console.log('Saved:', idleSteamPath);

    // 4. Quests Screenshot
    await page.evaluate(() => {
      switchTab('quests');
      renderQuests();
    });
    await settle();
    const questsPath = path.join(screenshotsDir, 'quests.png');
    await page.screenshot({ path: questsPath });
    console.log('Saved:', questsPath);

    console.log('All gallery screenshots updated successfully.');
  } finally {
    if (browser) await browser.close();
    if (server) await new Promise(resolve => server.close(resolve));
  }
})().catch(err => {
  console.error(err);
  process.exitCode = 1;
});
