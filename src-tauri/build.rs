fn main() {
    println!("cargo:rerun-if-changed=platform-auth.json");
    println!("cargo:rerun-if-env-changed=LOOTLAZY_PLATFORM_SERVICE_VERIFIED");
    if std::env::var_os("CARGO_FEATURE_PLATFORM_LOGIN").is_some() {
        let bytes = std::fs::read_to_string("platform-auth.json")
            .expect("Platform service configuration missing");
        let config: serde_json::Value = serde_json::from_str(bytes.trim_start_matches('\u{feff}'))
            .expect("Invalid platform service configuration");
        assert!(
            config["serviceUrl"]
                .as_str()
                .is_some_and(|url| url.starts_with("https://") && url.len() > 10),
            "Deploy the HTTPS platform service before enabling mandatory login"
        );
        assert!(
            std::env::var("LOOTLAZY_PLATFORM_SERVICE_VERIFIED").as_deref() == Ok("1"),
            "Verify live sign-in before enabling mandatory login"
        );
    }
    println!("cargo:rustc-check-cfg=cfg(steam_engine)");
    let engine = std::path::Path::new("../data/steam-engine/LootLazy.Steam.exe");
    println!("cargo:rerun-if-changed={}", engine.display());
    if engine.is_file() {
        println!("cargo:rustc-cfg=steam_engine");
    }
    tauri_build::build()
}
