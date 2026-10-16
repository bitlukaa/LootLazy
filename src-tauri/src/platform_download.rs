pub fn ensure_client(
    running: bool,
    auto_open: bool,
    manual_error: &str,
    launch: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if running {
        return Ok(());
    }
    if !auto_open {
        return Err(manual_error.into());
    }
    launch()
}
use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
fn download_url(platform: &str) -> Result<&'static str, String> {
    match platform {
        "discord" => Ok("https://discord.com/download"),
        "steam" => Ok("https://store.steampowered.com/about/"),
        _ => Err("Unsupported platform".into()),
    }
}
pub fn open(platform: &str) -> Result<(), String> {
    open_allowed(download_url(platform)?)
}
pub fn open_store(id: u32) -> Result<(), String> {
    if id == 0 {
        return Err("accounts_invalid".into());
    }
    open_allowed(&format!("https://store.steampowered.com/app/{id}/"))
}
pub fn open_allowed(url: &str) -> Result<(), String> {
    let wide = |value: &str| value.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let verb = wide("open");
    let target = wide(url);
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize <= 32 {
        Err("Could not open download page".into())
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn clients_only_launch_when_missing_and_explicitly_enabled() {
        use std::cell::Cell;
        let launches = Cell::new(0);
        for (running, enabled, expected) in [
            (false, false, 0),
            (true, false, 0),
            (true, true, 0),
            (false, true, 1),
        ] {
            launches.set(0);
            let result = super::ensure_client(running, enabled, "open_required", || {
                launches.set(launches.get() + 1);
                Ok(())
            });
            assert_eq!(launches.get(), expected);
            assert_eq!(result.is_err(), !running && !enabled);
        }
        assert_eq!(
            super::ensure_client(false, true, "open_required", || Err("launch_failed".into()))
                .unwrap_err(),
            "launch_failed"
        );
    }
    #[test]
    fn downloads_are_limited_to_official_platform_pages() {
        assert_eq!(
            super::download_url("discord").unwrap(),
            "https://discord.com/download"
        );
        assert_eq!(
            super::download_url("steam").unwrap(),
            "https://store.steampowered.com/about/"
        );
        for invalid in ["", "https://evil.example", "Steam", "discord/../"] {
            assert!(super::download_url(invalid).is_err());
        }
    }
}
