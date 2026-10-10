const fs = require('fs');
const path = require('path');

class SteamScanner {
  constructor() {
    this.libraries = [];
    this.discovered = false;
    this.customRoots = [];
    this.installCache = new Map();
    this.libraryCheckedAt = 0;
  }

  setCustomRoots(roots) {
    const next = roots.filter(r => typeof r === 'string');
    if (JSON.stringify(next) === JSON.stringify(this.customRoots)) return;
    this.customRoots = next; this.discovered = false; this.installCache.clear();
  }
  findSteamLibraries() {
    if (this.discovered && Date.now() - this.libraryCheckedAt < 30000) return this.libraries;

    const commonRoots = [...this.customRoots,
      'C:\\Program Files (x86)\\Steam',
      'C:\\Program Files\\Steam',
      'D:\\Steam',
      'D:\\SteamLibrary',
      'E:\\Steam',
      'E:\\SteamLibrary',
    ];

    const libraries = new Set();

    for (const root of commonRoots) {
      if (fs.existsSync(root)) {
        libraries.add(root);
        const vdfPath = path.join(root, 'steamapps', 'libraryfolders.vdf');
        if (fs.existsSync(vdfPath)) {
          try {
            const content = fs.readFileSync(vdfPath, 'utf8');
            const matches = content.match(/"path"\s+"([^"]+)"/g) || [];
            for (const m of matches) {
              const libPath = m.replace(/"path"\s+"/, '').replace(/"$/, '').replace(/\\\\/g, '\\');
              if (fs.existsSync(libPath)) {
                libraries.add(libPath);
              }
            }
          } catch (e) {
            // ignore vdf parse error
          }
        }
      }
    }

    this.libraries = Array.from(libraries);
    this.discovered = true;
    this.libraryCheckedAt = Date.now();
    return this.libraries;
  }

  findInstalledApp(steamAppId) {
    const previous = this.installCache.get(String(steamAppId));
    if (previous && Date.now() - previous.checkedAt < 30000) return previous.value;
    const value = this.discoverInstalledApp(steamAppId);
    this.installCache.set(String(steamAppId), {checkedAt:Date.now(), value});
    if (this.installCache.size > 2000) this.installCache.delete(this.installCache.keys().next().value);
    return value;
  }

  discoverInstalledApp(steamAppId) {
    if (!steamAppId) return null;
    const libs = this.findSteamLibraries();

    for (const lib of libs) {
      const manifestPath = path.join(lib, 'steamapps', `appmanifest_${steamAppId}.acf`);
      if (fs.existsSync(manifestPath)) {
        try {
          const content = fs.readFileSync(manifestPath, 'utf8');
          const installDirMatch = content.match(/"installdir"\s+"([^"]+)"/);
          if (installDirMatch) {
            const installDirName = installDirMatch[1];
            const gameDir = path.join(lib, 'steamapps', 'common', installDirName);
            if (fs.existsSync(gameDir)) {
              const exes = this.scanExecutables(gameDir);
              return {
                steamAppId,
                installDir: gameDir,
                executables: exes
              };
            }
          }
        } catch (e) {
          // ignore error
        }
      }
    }
    return null;
  }

  scanExecutables(dir, depth = 0, maxDepth = 2) {
    if (depth > maxDepth) return [];
    const results = [];
    try {
      const items = fs.readdirSync(dir, { withFileTypes: true });
      for (const item of items) {
        const full = path.join(dir, item.name);
        if (item.isFile() && item.name.toLowerCase().endsWith('.exe')) {
          // exclude common uninstallers/crashpad
          const lower = item.name.toLowerCase();
          if (!lower.includes('crash') && !lower.includes('unins') && !lower.includes('redist')) {
            results.push(item.name);
          }
        } else if (item.isDirectory() && (item.name.toLowerCase() === 'bin' || item.name.toLowerCase() === 'win64' || item.name.toLowerCase() === 'bin64' || item.name.toLowerCase() === 'binaries')) {
          const subExes = this.scanExecutables(full, depth + 1, maxDepth);
          for (const se of subExes) {
            results.push(`${item.name}/${se}`);
          }
        }
      }
    } catch (e) {
      // ignore
    }
    return results;
  }
}

module.exports = new SteamScanner();
