use std::{
    process::Command,
    sync::Mutex,
    time::{Duration, Instant},
};
static STATUS: Mutex<Option<(Instant, bool)>> = Mutex::new(None);
pub fn running() -> bool {
    let mut cached = match STATUS.lock() {
        Ok(cache) => cache,
        Err(_) => return false,
    };
    if let Some((time, running)) = *cached {
        if time.elapsed() < Duration::from_secs(3) {
            return running;
        }
    }
    let running = process_running();
    *cached = Some((Instant::now(), running));
    running
}
#[cfg(windows)]
fn process_running() -> bool {
    unsafe {
        use windows_sys::Win32::{
            Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
            System::Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
        };
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let length = entry
                    .szExeFile
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(entry.szExeFile.len());
                if String::from_utf16_lossy(&entry.szExeFile[..length])
                    .eq_ignore_ascii_case("steam.exe")
                {
                    found = true;
                    break;
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
        found
    }
}
#[cfg(not(windows))]
fn process_running() -> bool {
    false
}
pub fn ensure_open() -> Result<(), String> {
    if process_running() {
        return Ok(());
    }
    let exe = crate::executables::detected_roots()
        .into_iter()
        .map(|root| root.join("steam.exe"))
        .find(|path| path.is_file())
        .ok_or("steam_not_installed")?;
    let mut command = Command::new(exe);
    command.arg("-silent");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .spawn()
        .map_err(|_| "steam_launch_failed".to_string())?;
    if let Ok(mut cache) = STATUS.lock() {
        *cache = None;
    }
    Ok(())
}

pub fn installed() -> bool {
    crate::executables::detected_roots()
        .into_iter()
        .any(|root| root.join("steam.exe").is_file())
}
