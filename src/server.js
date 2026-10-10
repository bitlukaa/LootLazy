const http = require('http');
const fs = require('fs');
const path = require('path');

const resolver = require('./resolver');
const processManager = require('./processManager');
const artwork = require('./artwork');
const questFeed = require('./questFeed');
const QueueManager = require('./queueManager');
const queueManager = new QueueManager(processManager);
const questQueue = new QueueManager(processManager,{file:null,automatic:true});
processManager.on('session-ended',({session,reason})=>{if(session.questId&&['idle_timeout','quest_cancelled'].includes(reason))questFeed.closeQuest(session.questId,reason==='idle_timeout'?'finished':'cancelled');});
process.on('exit',()=>questQueue.dispose());
process.on('exit', () => queueManager.dispose());

const PORT = Number(process.env.QUESTRA_PORT || 3849);
const { validateAppId } = require('./validation');
const CACHE_FILE = path.join(__dirname, '..', 'data', 'detectable_cache.json');
const META_FILE = path.join(__dirname, '..', 'data', 'catalog_meta.json');
let detectableGames = [];
let catalogMeta = { lastUpdated: new Date().toISOString() };
// Popular / Shooter game IDs to prioritize at the top
const PRIORITY_GAME_IDS = [
  '363445589247131668',  // ROBLOX
  '1158877933042143272', // Counter-Strike 2
  '1402418703554842694', // Fortnite
  '542075586886107149',  // Apex Legends
  '762434991303950386',  // Genshin Impact
  '1121201675240210523', // Honkai: Star Rail
  '356875221078245376',  // Overwatch
  '357008169131606016',  // Rainbow Six Siege
  '1428223713928810500'  // The Line
];

// Load or fetch detectable games
async function initCatalog(force = false) {
  if (fs.existsSync(META_FILE)) {
    try { catalogMeta = JSON.parse(fs.readFileSync(META_FILE, 'utf8')); } catch (_) {}
  }

  if (!force && fs.existsSync(CACHE_FILE)) {
    try {
      detectableGames = JSON.parse(fs.readFileSync(CACHE_FILE, 'utf8'));
      console.log(`Loaded ${detectableGames.length} games from local cache.`);
      return;
    } catch (e) {
      console.warn('Failed to parse cache, will fetch fresh.');
    }
  }

  // Fetch from discord
  try {
    console.log('Fetching Discord detectable catalog...');
    const res = await fetch('https://discord.com/api/applications/detectable', { signal: AbortSignal.timeout(15000) });
    if (!res.ok) throw new Error(`Discord catalog HTTP ${res.status}`);
    if (res.ok) {
      const received = await res.json();
      if (!Array.isArray(received) || !received.length || received.some(game => typeof game.id !== 'string' || typeof game.name !== 'string')) throw new Error('Invalid catalog response');
      fs.writeFileSync(CACHE_FILE+'.tmp', JSON.stringify(received), 'utf8');
      fs.renameSync(CACHE_FILE+'.tmp', CACHE_FILE);
      detectableGames = received;
      catalogMeta = {
        lastUpdated: new Date().toISOString(),
        count: detectableGames.length
      };
      fs.writeFileSync(META_FILE, JSON.stringify(catalogMeta, null, 2), 'utf8');
      console.log(`Fetched and cached ${detectableGames.length} games.`);
      return;
    }
  } catch (err) {
    console.error('Failed to fetch detectable catalog from Discord:', err.message);
    if (force) throw err;
  }
}

// MIME types for static assets
const MIME_TYPES = {
  '.html': 'text/html',
  '.css': 'text/css',
  '.js': 'application/javascript',
  '.json': 'application/json',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.ico': 'image/x-icon',
  '.woff2': 'font/woff2'
};

const server = http.createServer(async (req, res) => {
  const requestUrl = new URL(req.url, 'http://127.0.0.1:' + PORT);
  const parsedUrl = { query: Object.fromEntries(requestUrl.searchParams) };
  const pathname = requestUrl.pathname;

  if (req.headers.origin && req.headers.origin !== 'http://127.0.0.1:' + PORT) { res.writeHead(403); res.end('Origin denied'); return; }
  if (req.headers.host !== '127.0.0.1:' + PORT && req.headers.host !== 'localhost:' + PORT) { res.writeHead(403); res.end('Host denied'); return; }
  res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
  res.setHeader('Cache-Control', 'no-store, no-cache, must-revalidate, proxy-revalidate');
  res.setHeader('Pragma', 'no-cache');
  res.setHeader('Expires', '0');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type');

  if (req.method === 'OPTIONS') {
    res.writeHead(204);
    res.end();
    return;
  }

  const sendJson = (status, data) => {
    res.writeHead(status, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify(data));
  };

  const getBody = () => new Promise((resolve, reject) => {
    let body = '';
    req.on('data', chunk => { body += chunk; if (body.length > 262144) { reject(new Error('Request too large')); req.destroy(); } });
    req.on('end', () => {
      try { resolve(JSON.parse(body || '{}')); }
      catch (_) { reject(new Error('Invalid JSON')); }
    });
  });

  // API Routes
  if (pathname.startsWith('/api/')) {
    try {
      if (pathname === '/api/artwork' && req.method === 'GET') {
        try {
          const image = process.env.QUESTRA_ARTWORK_OFFLINE === '1' ? artwork.read(parsedUrl.query.url) : await artwork.get(parsedUrl.query.url);
          if (!image) throw new Error('Offline artwork unavailable');
          res.writeHead(200, {'Content-Type':image.type === 'jpg' ? 'image/jpeg' : 'image/' + image.type, 'Cache-Control':'public, max-age=86400'});
          res.end(image.bytes);
        } catch { res.writeHead(404); res.end(); }
        return;
      }
      if (pathname === '/api/status' && req.method === 'GET') {
        const processStatus = processManager.getStatus();
        sendJson(200, {
          simulation: processStatus,
          queue: queueManager.getStatus(),questQueue:questQueue.getStatus()
        });
        return;
      }
      if (pathname === '/api/queue' && req.method === 'GET') { sendJson(200,queueManager.getStatus()); return; }
      if(pathname==='/api/play'&&req.method==='POST'){
        if(questQueue.active){sendJson(409,{error:'Stop the quest session first'});return;}
        const body=await getBody();let item;
        if(body.customProcess){const {validateExecutable}=require('./validation');const executableName=validateExecutable(body.executableName);item={appId:'999999999999999999',gameName:path.basename(executableName,'.exe'),executableName,durationMinutes:body.durationMinutes,activityEnabled:false};}
        else{const game=detectableGames.find(g=>g.id===body.appId);if(!game){sendJson(404,{error:'Game is not in the catalog'});return;}item={appId:game.id,gameName:game.name,executableName:body.executableName||resolver.resolve(game).primaryExecutable,iconHash:game.icon_hash||null,durationMinutes:body.durationMinutes};}
        if(Number(item.durationMinutes)===0){if(queueManager.active||processManager.getStatus().active){sendJson(409,{error:'Continuous sessions cannot be scheduled after another game'});return;}const result=await processManager.startSimulation(item.appId,item.gameName,item.executableName,0,item.iconHash,null,item.activityEnabled);sendJson(200,{success:true,...result,queue:queueManager.getStatus()});return;}
        const queue=await queueManager.playOrAppend(item);sendJson(queue.phase==='error'?500:200,{success:queue.phase!=='error',...processManager.getStatus(),queue});return;
      }
      if (pathname === '/api/queue/items' && req.method === 'POST') {
        const body=await getBody();
        if(body.action==='add') {
          const game=detectableGames.find(g=>g.id===body.appId);
          if(!game)throw new Error('Application is not in the Discord catalog');
          body.gameName=game.name; body.iconHash=game.icon_hash || game.cover_image_hash || null;
          body.executableName=body.executableName || resolver.resolve(game).primaryExecutable;
        }
        sendJson(200,{success:true,queue:queueManager.edit(body.action,body)}); return;
      }
      if(pathname==='/api/queue/start' && req.method==='POST') {
        if(questQueue.active){sendJson(409,{error:'Stop the quest queue first'});return;}
        queueManager.automatic=true;queueManager.save();
        const queue=await queueManager.start();
        sendJson(queue.phase==='error'?500:200,{success:queue.phase!=='error',queue}); return;
      }
      if(pathname==='/api/queue/cancel' && req.method==='POST') {
        sendJson(200,{success:true,queue:await queueManager.cancel()}); return;
      }
      if(pathname==='/api/queue/next' && req.method==='POST') {const queue=await queueManager.next();sendJson(queue.phase==='error'?500:200,{success:queue.phase!=='error',queue});return;}

      if (pathname === '/api/quests' && req.method === 'GET') {
        const data=parsedUrl.query.refresh==='true'?await questFeed.refresh():questFeed.get();
        const quests=data.quests.map(q=>{const game=detectableGames.find(g=>g.id===q.appId);const fallback=q.appId?{id:q.appId,name:q.gameName,executables:[],catalogMissing:true}:null;return {...q,game:game?{...game,resolution:resolver.resolve(game)}:fallback?{...fallback,resolution:resolver.resolve(fallback)}:null};});
          const history=processManager.getHistory();
          const closed=questFeed.closedStates();
          for(const quest of quests){const local=closed[quest.id];quest.localClosedReason=local&&local.reason!=='open'?local.reason:null;quest.localFinished=local?local.reason!=='open':history.some(s=>s.questId===quest.id&&s.reason==='idle_timeout');}
          quests.sort((a,b)=>Number(a.localFinished)-Number(b.localFinished));
          sendJson(200,{...data,quests});return;
      }
        if(pathname==='/api/quests/start' && req.method==='POST'){
          if(queueManager.active){sendJson(409,{error:'Stop the queue first'});return;}
          const {questId}=await getBody();
          const quest=questFeed.get().quests.find(q=>q.id===questId);
          if(!quest){sendJson(404,{error:'Quest is no longer available'});return;}
          const local=questFeed.closedStates()[quest.id];if(local&&local.reason!=='open'){sendJson(409,{error:'Quest is locally closed'});return;}
          const game=detectableGames.find(g=>g.id===quest.appId)||{id:quest.appId,name:quest.gameName,executables:[]};
          const resolution=resolver.resolve(game);
          if(!game.id){sendJson(422,{error:'No application ID for this quest'});return;}
          const queue=await questQueue.appendQuest({appId:game.id,gameName:game.name,executableName:resolution.primaryExecutable,durationMinutes:15.5,iconHash:game.icon_hash||null,questId:quest.id});
          sendJson(queue.phase==='error'?500:200,{success:queue.phase!=='error',...processManager.getStatus(),questQueue:queue});return;
        }
        if(pathname==='/api/quests/reopen'&&req.method==='POST'){
          const {questId}=await getBody();
          if(!questFeed.get().quests.some(q=>q.id===questId)){sendJson(404,{error:'Quest is no longer available'});return;}
          if(processManager.getStatus().session?.questId===questId||questQueue.active&&questQueue.items.some(q=>q.questId===questId&&['queued','starting','running','waiting','ready_next'].includes(q.state))){sendJson(409,{error:'Quest is already running or queued'});return;}
          questFeed.reopenQuest(questId);sendJson(200,{success:true});return;
        }
        if(pathname==='/api/quests/cancel'&&req.method==='POST'){
          const {questId}=await getBody();
          if(!questFeed.get().quests.some(q=>q.id===questId)&&!questQueue.items.some(q=>q.questId===questId)){sendJson(404,{error:'Quest not found'});return;}
          const queue=await questQueue.cancelQuest(questId);questFeed.closeQuest(questId,'cancelled');
          sendJson(200,{success:true,questQueue:queue,...processManager.getStatus()});return;
        }
      if(pathname==='/api/quests/clear' && req.method==='POST'){sendJson(200,{success:true,...questFeed.clear()});return;}
      if (pathname === '/api/settings' && req.method === 'GET') { resolver.loadData(); sendJson(200,resolver.overrides.settings || {}); return; }
      if (pathname === '/api/settings' && req.method === 'POST') {
        const body = await getBody(); resolver.loadData();
        const settings = {...resolver.overrides.settings};
        if (typeof body.scan_local_steam === 'boolean') settings.scan_local_steam = body.scan_local_steam;
        if (body.steam_paths !== undefined) {
          if (!Array.isArray(body.steam_paths) || body.steam_paths.length > 8) throw new Error('Invalid Steam folders');
          settings.steam_paths = body.steam_paths.map(folder => {
            if (typeof folder !== 'string' || !path.isAbsolute(folder) || !fs.existsSync(path.join(folder, 'steamapps'))) throw new Error('Choose a Steam folder containing steamapps');
            return path.resolve(folder);
          });
        }
        resolver.overrides.settings = settings;
        resolver.saveOverrides(); resolver.loadData(); sendJson(200,{success:true}); return;
      }
      if (pathname === '/api/games' && req.method === 'GET') {
        const q = (parsedUrl.query.q || '').toLowerCase().trim();
        const category = (parsedUrl.query.category || '').toLowerCase().trim();
        const limit = Math.max(1, Math.min(100, parseInt(parsedUrl.query.limit, 10) || 50));
        const offset=Math.max(0,parseInt(parsedUrl.query.offset,10)||0);

        let filtered = detectableGames;

        // Filter by category/genre
        if (category && category !== 'all') {
          filtered = filtered.filter(g => {
            const themes = (g.themes || []).map(t => t.toLowerCase());
            if (category === 'shooter') {
              return themes.includes('action') || themes.includes('warfare') || g.name.toLowerCase().includes('strike') || g.name.toLowerCase().includes('call of duty') || g.name.toLowerCase().includes('apex');
            }
            if (category === 'action') return themes.includes('action');
            if (category === 'rpg') return themes.includes('role-playing') || themes.includes('fantasy');
            if (category === 'strategy') return themes.includes('strategy');
            return true;
          });
        }

        // Filter by search query
        if (q) {
          filtered = filtered.filter(g => {
            if (g.name && g.name.toLowerCase().includes(q)) return true;
            if (g.id && g.id.includes(q)) return true;
            if (Array.isArray(g.aliases) && g.aliases.some(a => a.toLowerCase().includes(q))) return true;
            return false;
          });
        } else if (!category || category === 'all') {
          // Sort priority popular shooter games to top
          const priority = [];
          const rest = [];
          for (const g of filtered) {
            if (PRIORITY_GAME_IDS.includes(g.id)) priority.push(g);
            else rest.push(g);
          }
          filtered = [...priority, ...rest];
        }

        const total=filtered.length;
        const results = filtered.slice(offset, offset+limit);

        const enriched = results.map(g => ({
          ...g,
          resolution: resolver.resolve(g)
        }));

        sendJson(200, { games: enriched,total,hasMore:offset+results.length<total });
        return;
      }

      if (pathname === '/api/refresh-catalog' && req.method === 'POST') {
        await initCatalog(true);
        sendJson(200, {
          success: true,
          count: detectableGames.length,
          lastUpdated: catalogMeta.lastUpdated
        });
        return;
      }

      if (pathname === '/api/history' && req.method === 'GET') {
        const history = processManager.getHistory();
        sendJson(200, { history });
        return;
      }

      if (pathname === '/api/resolve' && req.method === 'GET') {
        const id = parsedUrl.query.id;
        const game = detectableGames.find(g => g.id === id) || { id, name: 'Game' };
        sendJson(200, resolver.resolve(game));
        return;
      }

      if (pathname === '/api/override' && req.method === 'POST') {
        const { appId, executableName, resetAll } = await getBody();
        if (resetAll === true) {
          resolver.loadData(); resolver.overrides.custom_executables = {}; resolver.saveOverrides();
          sendJson(200, {success:true}); return;
        }
        resolver.setCustomExecutable(appId, executableName);
        const resolution = resolver.resolve(detectableGames.find(g => g.id === appId) || appId);
        sendJson(200, { success: true, resolution });
        return;
      }

      if (pathname === '/api/start' && req.method === 'POST') {
          if(queueManager.active||questQueue.active) {sendJson(409,{error:'Stop the queue before starting a game manually'});return;}
        const { appId, gameName, executableName, durationMinutes, iconHash } = await getBody();
          if (!appId) {
            sendJson(400, { error: 'appId is required.' });
          return;
        }
        const status = await processManager.startSimulation(
          appId,
          gameName,
          executableName,
          durationMinutes,
          iconHash
        );
        sendJson(200, { success: true, ...status });
        return;
      }

      if (pathname === '/api/stop' && req.method === 'POST') {
        if(queueManager.active)await queueManager.cancel();
        if(questQueue.active){const pending=questQueue.items.filter(q=>!['completed','cancelled'].includes(q.state)).map(q=>q.questId);await questQueue.cancel();for(const id of pending)questFeed.closeQuest(id,'cancelled');}
        const result = await processManager.stopSimulation('user_requested');
        sendJson(200, result);
        return;
      }
      if(pathname==='/api/stop-current'&&req.method==='POST'){
        if(questQueue.active){sendJson(409,{error:'Use the quest card to stop this session'});return;}
        if(queueManager.active)await queueManager.stopCurrent();else await processManager.stopSimulation('user_requested');
        sendJson(200,{success:true,queue:queueManager.getStatus(),...processManager.getStatus()});return;
      }

      if (pathname === '/api/catalog-stats' && req.method === 'GET') {
        const emptyCount = detectableGames.filter(g => !g.executables || g.executables.length === 0).length;
        sendJson(200, {
          totalGames: detectableGames.length,
          emptyExecutablesCount: emptyCount,
          solvedCatalogCount: Object.keys(resolver.catalog.games || {}).length,
          userOverridesCount: Object.keys(resolver.overrides.custom_executables || {}).length,
          lastUpdated: catalogMeta.lastUpdated
        });
        return;
      }

      sendJson(404, { error: 'Endpoint not found' });
      return;
    } catch (e) {
      console.error('API Error:', e);
      sendJson(500, { error: e.message });
      return;
    }
  }

  // Static File Serving
  const publicRoot = path.resolve(__dirname, '..', 'public');
  let filePath;
  try { filePath = path.resolve(publicRoot, '.' + decodeURIComponent(pathname === '/' ? '/index.html' : pathname)); } catch { res.writeHead(400); res.end(); return; }
  if (!filePath.startsWith(publicRoot + path.sep)) { res.writeHead(403); res.end(); return; }

  fs.stat(filePath, (err, stats) => {
    if (err || !stats.isFile()) {
      res.writeHead(404); res.end('Asset not found'); return;
    }
    const contentType = MIME_TYPES[path.extname(filePath).toLowerCase()] || 'application/octet-stream';
    fs.readFile(filePath, (readErr, content) => {
      if (readErr) {
        res.writeHead(500);
        res.end('Error loading asset');
        return;
      }
      res.writeHead(200, { 'Content-Type': contentType });
      res.end(content);
    });
  });
});

function start() {
  return new Promise(async (resolve, reject) => {
    try {
      await initCatalog();
      server.listen(PORT, '127.0.0.1', () => {
        console.log(`=======================================================`);
        console.log(`  🎮 Questro`);
        console.log(`  🌐 URL: http://127.0.0.1:${PORT}`);
        console.log(`=======================================================`);
        resolve(PORT);
      });
      server.on('error', (err) => {
        if (err.code === 'EADDRINUSE') {
          reject(new Error(`Port ${PORT} is already in use. Close the older Questro instance.`));
        } else {
          reject(err);
        }
      });
    } catch (e) {
      reject(e);
    }
  });
}

if (require.main === module) {
  start().catch(err => { console.error(err.message); process.exit(1); });
}

module.exports = { start, server, PORT, queueManager, questQueue, questFeed };
