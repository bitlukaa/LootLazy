const fs=require('fs');const path=require('path');
(async()=>{
 const root=path.resolve(__dirname,'..');const output=path.join(root,'dist','Questro');
 fs.mkdirSync(output,{recursive:true});
 const runtime = path.join(root,'node_modules','electron','dist');
 const exe=path.join(output,'Questro.exe');
 if (fs.existsSync(runtime)) { fs.cpSync(runtime,output,{recursive:true});fs.renameSync(path.join(output,'electron.exe'),exe); }
 else if (!fs.existsSync(exe)) throw new Error('No runtime found. Run npm ci before the first build.');
 const appRoot=path.join(output,'resources','app');fs.mkdirSync(appRoot,{recursive:true});
 for(const name of ['src','public','runner'])fs.cpSync(path.join(root,name),path.join(appRoot,name),{recursive:true});
 if (!fs.existsSync(path.join(appRoot,'data')) && fs.existsSync(path.join(root,'dist','Questra','resources','app','data'))) fs.cpSync(path.join(root,'dist','Questra','resources','app','data'),path.join(appRoot,'data'),{recursive:true});
 // Preserve the portable user's library, overrides, quests, and history on rebuild.
 if(!fs.existsSync(path.join(appRoot,'data')))fs.cpSync(path.join(root,'data'),path.join(appRoot,'data'),{recursive:true});
 fs.copyFileSync(path.join(root,'package.json'),path.join(appRoot,'package.json'));
 fs.copyFileSync(path.join(root,'README.md'),path.join(appRoot,'README.md'));
 fs.copyFileSync(path.join(root,'public','assets','LUCIDE-LICENSE.txt'),path.join(output,'LUCIDE-LICENSE.txt'));
 fs.copyFileSync(path.join(root,'public','assets','DISCORD-QUEST-HELPER-LICENSE.txt'),path.join(output,'DISCORD-QUEST-HELPER-LICENSE.txt'));
 if (fs.existsSync(runtime)) await require('rcedit')(exe,{icon:path.join(root,'Questro.ico'),'version-string':{ProductName:'Questro',FileDescription:'Questro'},'file-version':require('../package.json').version,'product-version':require('../package.json').version});
 const files=[];for(const dir of ['src','public']){const walk=p=>{for(const item of fs.readdirSync(p,{withFileTypes:true})){const full=path.join(p,item.name);if(item.isDirectory())walk(full);else files.push({path:path.relative(appRoot,full),sha256:require('crypto').createHash('sha256').update(fs.readFileSync(full)).digest('hex')});}};walk(path.join(appRoot,dir));}
 fs.writeFileSync(path.join(output,'build-manifest.json'),JSON.stringify({version:require('../package.json').version,builtAt:new Date().toISOString(),files},null,2));
 const compiler = path.join(process.env.WINDIR || 'C:\\Windows','Microsoft.NET','Framework64','v4.0.30319','csc.exe');
 if (fs.existsSync(compiler)) require('child_process').execFileSync(compiler,['/nologo','/target:winexe','/r:System.Windows.Forms.dll','/win32icon:'+path.join(root,'Questro.ico'),'/out:'+path.join(root,'Questro.exe'),path.join(root,'src','Launcher.cs')]);
 console.log('Built '+exe);
})().catch(e=>{console.error(e);process.exitCode=1});
