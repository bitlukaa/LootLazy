const {spawn}=require('child_process');const path=require('path');const fs=require('fs');
const exe=path.resolve(__dirname,'../dist/Questro/Questro.exe');
if(!fs.existsSync(exe)){console.error('Questro build not found. Run npm ci then npm run build.');process.exit(1);}
const child=spawn(exe,[],{cwd:path.dirname(exe),stdio:'ignore',detached:true,windowsHide:true});child.on('error',e=>{console.error(e.message);process.exitCode=1;});child.unref();
