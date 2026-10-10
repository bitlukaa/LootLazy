const fs=require('fs'),path=require('path'),os=require('os');
const pm=require('../src/processManager');const dir=fs.mkdtempSync(path.join(os.tmpdir(),'questro-detection-'));
// Explicitly gated; do not run the native-window mode without user authorization.
pm.nativeDetectionEnabled=process.argv.includes('--native-test');
pm.gamesRoot=path.join(dir,'games');pm.historyPath=path.join(dir,'history.json');pm.presence=new(require('../src/discordPresence').DiscordPresence)({paths:['\\\\?\\pipe\\questro-isolated-native-test-'+process.pid],timeout:100});
pm.on('session-ended',()=>{fs.rmSync(dir,{recursive:true,force:true});console.log('Native probe stopped.');});
pm.startSimulation('1158877933042143272','Counter-Strike 2','win64/cs2.exe',1).then(result=>console.log(JSON.stringify({pid:result.session.pid,executable:result.session.executablePath,durationSeconds:60}))).catch(e=>{console.error(e);process.exitCode=1;});
