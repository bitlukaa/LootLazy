const test=require('node:test');const assert=require('node:assert/strict');const fs=require('fs');const path=require('path');const os=require('os');
const {ArtworkCache,validatedUrl}=require('../src/artwork');
function fixture(prefix){return fs.mkdtempSync(path.join(os.tmpdir(),prefix));}
function cleanup(folder,prefix){const absolute=path.resolve(folder);assert.ok(absolute.startsWith(path.resolve(os.tmpdir(),prefix)));fs.rmSync(absolute,{recursive:true,force:true});}
test('artwork source validation rejects arbitrary and credential URLs',()=>{
 assert.ok(validatedUrl('https://cdn.discordapp.com/app-icons/12345/hash.png'));
 for(const url of ['http://cdn.discordapp.com/app-icons/12345/hash.png','https://example.com/image.png','https://cdn.discordapp.com@localhost/a','https://cdn.discordapp.com/other/image.png'])assert.throws(()=>validatedUrl(url));
});
test('artwork downloads are shared and images remain usable offline',async()=>{
 const folder=fixture('questro-art-');const cache=new ArtworkCache(folder);const originalFetch=global.fetch;let calls=0;const png=fs.readFileSync(path.resolve('public/assets/questro-logo.png'));
 try{global.fetch=async()=>{calls++;return new Response(png)};const url='https://cdn.discordapp.com/app-icons/12345/test.png';const [one,two]=await Promise.all([cache.get(url),cache.get(url)]);assert.equal(calls,1);assert.deepEqual(one.bytes,two.bytes);global.fetch=async()=>{throw new Error('offline')};assert.deepEqual((await cache.get(url)).bytes,png);}finally{global.fetch=originalFetch;cleanup(folder,'questro-art-');}
});
test('custom Steam roots discover nested executables and cache scans',()=>{
 const folder=fixture('questro-steam-');const scanner=require('../src/steamScanner');const exeDir=path.join(folder,'steamapps/common/Test Game/Binaries/Win64');fs.mkdirSync(exeDir,{recursive:true});fs.writeFileSync(path.join(folder,'steamapps/appmanifest_12345.acf'),'"installdir" "Test Game"');fs.writeFileSync(path.join(exeDir,'Game.exe'),'fixture');
 try{scanner.setCustomRoots([folder]);const game=scanner.findInstalledApp('12345');assert.ok(game.executables.includes('Binaries/Win64/Game.exe'));assert.strictEqual(scanner.findInstalledApp('12345'),game);}finally{scanner.setCustomRoots([]);cleanup(folder,'questro-steam-');}
});
