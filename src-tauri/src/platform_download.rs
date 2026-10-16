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
