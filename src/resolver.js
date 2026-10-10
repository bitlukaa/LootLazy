const fs = require('fs');
const path = require('path');
const steamScanner = require('./steamScanner');
const { validateExecutable, validateAppId } = require('./validation');

class ExecutableResolver {
  constructor() {
    this.catalogPath = path.join(__dirname, '..', 'data', 'questra_catalog.json');
    this.overridesPath = path.join(__dirname, '..', 'data', 'questra_overrides.json');
    this.catalog = null;
    this.overrides = null;
    this.fileSignatures = {};
    this.loadData();
  }

  loadData() {
    const signature = file => { try { const stat = fs.statSync(file); return `${stat.mtimeMs}:${stat.size}`; } catch { return "missing"; } };
    const current = signature(this.catalogPath) + "|" + signature(this.overridesPath);
    if (this.fileSignatures.current === current && this.catalog && this.overrides) return;
    this.fileSignatures.current = current;
    try {
      if (fs.existsSync(this.catalogPath)) {
        this.catalog = JSON.parse(fs.readFileSync(this.catalogPath, 'utf8'));
      } else {
        this.catalog = { games: {} };
      }
    } catch (e) {
      this.catalog = { games: {} };
    }

    try {
      if (fs.existsSync(this.overridesPath)) {
        this.overrides = JSON.parse(fs.readFileSync(this.overridesPath, 'utf8'));
      } else {
        this.overrides = { saved_games: [], custom_executables: {}, settings: {} };
      }
    } catch (e) {
      this.overrides = { saved_games: [], custom_executables: {}, settings: {} };
    }
    steamScanner.setCustomRoots(this.overrides.settings?.steam_paths || []);
  }

  saveOverrides() {
    try {
      fs.writeFileSync(this.overridesPath, JSON.stringify(this.overrides, null, 2), 'utf8');
    } catch (e) {
      throw e;
    }
  }

  setCustomExecutable(appId, executableName) {
    validateAppId(appId);
    if (executableName) validateExecutable(executableName);
    this.loadData();
    if (!this.overrides.custom_executables) this.overrides.custom_executables = {};
    if (executableName && executableName.trim()) {
      this.overrides.custom_executables[appId] = executableName.trim().replace(/\\/g, '/');
    } else {
      delete this.overrides.custom_executables[appId];
    }
    this.saveOverrides();
    return this.resolve(appId);
  }

  resolve(game) {
    this.loadData();
    const appId = typeof game === 'object' ? game.id : String(game);
    const name = typeof game === 'object' ? game.name : 'Unknown Game';
    const executables = typeof game === 'object' && Array.isArray(game.executables) ? game.executables : [];
    const thirdPartySkus = typeof game === 'object' && Array.isArray(game.third_party_skus) ? game.third_party_skus : [];

    // 1. Check User Custom Override
    if (this.overrides.custom_executables && this.overrides.custom_executables[appId]) {
      const customExe = this.overrides.custom_executables[appId];
      return {
        appId,
        gameName: name,
        primaryExecutable: customExe,
        executables: [customExe],
        status: 'custom_override',
        statusColor: 'amber',
        isReliable: false,
        source: 'User Manual Configuration'
      };
    }

    // 2. Check Official Discord executables (win32)
    const win32Exes = executables
      .filter(e => !e.os || e.os === 'win32')
      .map(e => e.name)
      .filter(name=>{try{validateExecutable(name);return true;}catch{return false;}});

    if (win32Exes.length > 0) {
      return {
        appId,
        gameName: name,
        primaryExecutable: win32Exes[0],
        executables: win32Exes,
        status: 'official_verified',
        statusColor: 'emerald',
        isReliable: true,
        source: 'Discord Detectable Registry'
      };
    }

    // 4. Check Local Steam Library if SKU is present
    const steamSku = thirdPartySkus.find(s => s.distributor === 'steam');
    if (this.overrides.settings?.scan_local_steam !== false && steamSku && steamSku.id) {
      const localSteam = steamScanner.findInstalledApp(steamSku.id);
      if (localSteam && localSteam.executables.length > 0) {
        return {
          appId,
          gameName: name,
          primaryExecutable: localSteam.executables[0],
          executables: localSteam.executables,
          status: 'local_steam_discovered',
          statusColor: 'cyan',
          isReliable: true,
          source: `Steam Local Install: ${localSteam.installDir}`
        };
      }
    }

    // 3. Check Curated Questra Catalog
    if (this.catalog.games && this.catalog.games[appId]) {
      const catalogEntry = this.catalog.games[appId];
      const catExes = catalogEntry.executables || [];
      if (catExes.length > 0) {
        return {
          appId,
          gameName: name || catalogEntry.name,
          primaryExecutable: null,
          executables: catExes,
          status: 'unverified_catalog',
          statusColor: 'purple',
          isReliable: false,
          source: catalogEntry.source || 'Questra Registry'
        };
      }
    }

    return {appId,gameName:name,primaryExecutable:null,executables:[],status:'unresolved',statusColor:'muted',isReliable:false,source:'No known Windows executable'};
  }
}

module.exports = new ExecutableResolver();
