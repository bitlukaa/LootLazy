use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};
fn digest(file: &mut File) -> Result<String, String> {
    let size = file
        .metadata()
        .map_err(|_| "steam_runtime_untrusted")?
        .len();
    if !(512..=8 * 1024 * 1024).contains(&size) {
        return Err("steam_runtime_untrusted".into());
    }
    let mut bytes = Vec::new();
    (&mut *file)
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "steam_runtime_untrusted")?;
    if !bytes.starts_with(b"MZ") {
        return Err("steam_runtime_untrusted".into());
    }
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
pub fn lock_verified(path: &Path, expected: Option<&str>) -> Result<(File, String), String> {
    #[cfg(windows)]
    let mut file = {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(path)
            .map_err(|_| "steam_runtime_untrusted")?
    };
    #[cfg(not(windows))]
    let mut file = File::open(path).map_err(|_| "steam_runtime_untrusted")?;
    let hash = digest(&mut file)?;
    if expected.is_some_and(|expected| expected != hash) {
        return Err("steam_runtime_untrusted".into());
    }
    static VERIFIED: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    let cache = VERIFIED.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()));
    let known = cache
        .lock()
        .map_err(|_| "steam_runtime_untrusted")?
        .contains(&hash);
    if !known {
        verify_publisher(path)?;
        let mut verified = cache.lock().map_err(|_| "steam_runtime_untrusted")?;
        if verified.len() >= 64 {
            verified.clear();
        }
        verified.insert(hash.clone());
    }
    Ok((file, hash))
}
#[cfg(windows)]
fn verify_publisher(path: &Path) -> Result<(), String> {
    use std::{
        os::windows::process::CommandExt,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let engine =
        std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:/Windows".into()))
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let script="$s = Get-AuthenticodeSignature -LiteralPath $env:QUESTRO_RUNTIME_PATH; if ($s.Status -eq 'Valid' -and $s.SignerCertificate.Subject -match '(^|,\\s*)O=Valve Corp\\.(,|$)') { exit 0 } else { exit 1 }";
    let mut child = Command::new(engine)
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("QUESTRO_RUNTIME_PATH", path)
        .env_remove("PSModulePath")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|_| "steam_runtime_untrusted")?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().map_err(|_| "steam_runtime_untrusted")? {
            return if status.success() {
                Ok(())
            } else {
                Err("steam_runtime_untrusted".into())
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("steam_runtime_untrusted".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
#[cfg(not(windows))]
fn verify_publisher(_: &Path) -> Result<(), String> {
    Err("steam_runtime_untrusted".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tampered_or_unrecognized_runtimes_are_rejected() {
        let path =
            std::env::temp_dir().join(format!("questro-runtime-{}.dll", rand::random::<u64>()));
        std::fs::write(&path, b"untrusted").unwrap();
        assert!(lock_verified(&path, None).is_err());
        let mut bytes = vec![0u8; 512];
        bytes[..2].copy_from_slice(b"MZ");
        std::fs::write(&path, &bytes).unwrap();
        assert!(lock_verified(&path, Some(&"0".repeat(64))).is_err());
        assert!(lock_verified(&path, None).is_err());
        let _ = std::fs::remove_file(path);
    }
}
