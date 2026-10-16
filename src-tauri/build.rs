fn main() {
    println!("cargo:rustc-check-cfg=cfg(steam_engine)");
    let engine = std::path::Path::new("../data/steam-engine/LootLazy.Steam.exe");
    println!("cargo:rerun-if-changed={}", engine.display());
    if engine.is_file() {
        println!("cargo:rustc-cfg=steam_engine");
    }
    tauri_build::build()
}
