const fs=require('fs'),path=require('path'),os=require('os'),assert=require('node:assert/strict'),{execFileSync}=require('child_process');
const manager=require('../src/processManager');
manager.presence=new(require('../src/discordPresence').DiscordPresence)({paths:['\\\\?\\pipe\\questro-isolated-window-check-'+process.pid],timeout:100});
(async()=>{const dir=fs.mkdtempSync(path.join(os.tmpdir(),'questro-window-'));manager.gamesRoot=path.join(dir,'games');manager.historyPath=path.join(dir,'history.json');try{
 const result=await manager.startSimulation('123456789','Questro window verification','window-check.exe',1);
 let window;
 const probe=`Add-Type -TypeDefinition @'
using System;using System.Runtime.InteropServices;
public class SessionWindowProbe {
 public delegate bool Callback(IntPtr handle,IntPtr param);
 [DllImport("user32.dll")]public static extern bool EnumWindows(Callback callback,IntPtr param);
 [DllImport("user32.dll")]public static extern uint GetWindowThreadProcessId(IntPtr handle,out uint pid);
 [DllImport("user32.dll")]public static extern bool IsWindowVisible(IntPtr handle);
 [DllImport("user32.dll")]public static extern bool IsIconic(IntPtr handle);
 [DllImport("user32.dll")]public static extern IntPtr GetWindow(IntPtr handle,uint kind);
 [DllImport("user32.dll",EntryPoint="GetWindowLongPtrW")]public static extern IntPtr GetWindowLongPtr(IntPtr handle,int index);
 public static int[] Count(uint target){int total=0,visible=0,minimized=0,taskbar=0;EnumWindows((h,p)=>{uint pid;GetWindowThreadProcessId(h,out pid);if(pid==target){total++;if(IsWindowVisible(h)){visible++;if(IsIconic(h))minimized++;if(GetWindow(h,4)==IntPtr.Zero||(GetWindowLongPtr(h,-20).ToInt64()&0x40000)!=0)taskbar++;}}return true;},IntPtr.Zero);return new int[]{total,visible,minimized,taskbar};}
}
'@
$taskProcess=Get-Process -Id ${result.session.pid};$counts=[SessionWindowProbe]::Count(${result.session.pid});[pscustomobject]@{pid=$taskProcess.Id;name=$taskProcess.ProcessName;windows=$counts[0];nativeWindows=$counts[1];minimizedWindows=$counts[2];taskbarEligibleWindows=$counts[3]}|ConvertTo-Json -Compress`;
 for(let attempt=0;attempt<3;attempt++){await new Promise(r=>setTimeout(r,500));window=JSON.parse(execFileSync('powershell.exe',['-NoProfile','-Command',probe],{encoding:'utf8',windowsHide:true}));assert.ok(window.nativeWindows>0);assert.equal(window.nativeWindows,window.minimizedWindows,'Every native game window must stay minimized');assert.equal(window.taskbarEligibleWindows,0,'Owned game windows must not add taskbar entries');assert.equal(window.name,'window-check');}
 fs.writeFileSync('docs/RUNNER_WINDOW_VERIFICATION.json',JSON.stringify({passed:true,...window},null,2));console.log('Native runner verified: minimized owned window, no taskbar entry.');
 }finally{await manager.stopSimulation();assert.ok(path.resolve(dir).startsWith(path.resolve(os.tmpdir(),'questro-window-')));fs.rmSync(dir,{recursive:true,force:true});}})().catch(error=>{console.error(error);process.exitCode=1});
