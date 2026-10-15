const {spawn}=require('node:child_process'),fs=require('fs'),path=require('path');
const root=path.resolve(__dirname,'..'),state=fs.mkdtempSync(path.join(root,'data/accounts-startup-'));
const child=spawn(path.join(root,'data/steam-engine/LootLazy.Steam.exe'),['--bridge',state,String(process.pid)],{windowsHide:true,stdio:['pipe','pipe','pipe'],env:{...process.env,DOTNET_BUNDLE_EXTRACT_BASE_DIR:path.join(root,'data/engine-test-runtime')}});
let ready=false,output='',step=0,accountId='';const timer=setTimeout(()=>{child.kill();console.error('Actual engine startup timed out');process.exitCode=1;},100000);
child.stdout.on('data',chunk=>{output+=chunk.toString();let newline;while((newline=output.indexOf('\n'))>=0){const frame=JSON.parse(output.slice(0,newline));output=output.slice(newline+1);if(frame.ready){ready=true;child.stdin.write(JSON.stringify({id:1,protocol:1,command:'status',body:{}})+'\n');}else{
 if(!frame.ok){console.error('Startup fixture operation failed: '+frame.error);process.exitCode=1;child.stdin.end();return;}
 if(step===0){if(frame.result.accounts.length)throw Error('Expected isolated state');step=1;child.stdin.write(JSON.stringify({id:2,protocol:1,command:'add',body:{label:'Offline fixture',login:'lootlazy_offline_fixture'}})+'\n');}
 else if(step===1){accountId=frame.result.accounts[0].id;if(frame.result.accounts[0].active)throw Error('Registration must not sign in automatically');step=2;child.stdin.write(JSON.stringify({id:3,protocol:1,command:'remove',body:{id:accountId}})+'\n');}
 else{if(frame.result.accounts.length)throw Error('Bot removal incomplete');child.stdin.end();}
}}});
child.on('exit',code=>{clearTimeout(timer);if(!ready||code!==0){console.error('Actual engine startup/exit failed: '+code);process.exitCode=1;}else console.log('Actual ASF engine initialized and shut down through private pipes; no Steam accounts or credentials used.');});child.on('error',error=>{clearTimeout(timer);console.error(error.message);process.exitCode=1;});
