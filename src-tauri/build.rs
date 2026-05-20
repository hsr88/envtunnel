use std::path::PathBuf;

fn main() {
    // Search for legacy_stdio_definitions.lib in known VS2022 locations
    let program_files = std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".to_string());
    let vs_base = PathBuf::from(&program_files)
        .join(r"Microsoft Visual Studio\2022\Community\VC\Tools\MSVC");

    if let Ok(entries) = std::fs::read_dir(&vs_base) {
        for entry in entries.flatten() {
            let lib_path = entry.path().join(r"lib\onecore\x64");
            if lib_path.join("legacy_stdio_definitions.lib").exists() {
                println!("cargo:rustc-link-search=native={}", lib_path.display());
                break;
            }
        }
    }

    tauri_build::build()
}
