param([Parameter(Mandatory=$true)][int]$TargetProcessId)
$ErrorActionPreference='Stop'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public class QuestroOwnedWindowProbe {
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
$taskProcess=Get-Process -Id $TargetProcessId
$counts=[QuestroOwnedWindowProbe]::Count($TargetProcessId)
[pscustomobject]@{pid=$taskProcess.Id;windows=$counts[0];nativeWindows=$counts[1];minimizedWindows=$counts[2];taskbarEligibleWindows=$counts[3]}|ConvertTo-Json -Compress
