use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Process LALRPOP grammar
    lalrpop::process_root().unwrap();

    // Copy theme files and tasks.toml to the output directory
    let out_dir = env::var("OUT_DIR").unwrap();
    // OUT_DIR is something like target/x86_64-pc-windows-gnu/release/build/wolfy-xxx/out
    // We need to go up to the target profile directory (release or debug)
    let out_path = Path::new(&out_dir);

    // Navigate up: out -> wolfy-xxx -> build -> release -> target-triple -> target
    // We want: target/x86_64-pc-windows-gnu/release/
    if let Some(profile_dir) = out_path.ancestors().nth(3)
    // up 3 levels from OUT_DIR
    {
        // List of theme files to copy
        let theme_files = [
            "default.rasi",          // Legacy/fallback
            "launcher.rasi",         // Launcher window theme
            "theme_picker.rasi",     // Theme picker window theme
            "wallpaper_picker.rasi", // Wallpaper picker window theme
        ];

        for file in &theme_files {
            let src = Path::new(file);
            let dst = profile_dir.join(file);

            if src.exists() {
                println!("cargo:rerun-if-changed={}", file);
                if let Err(e) = fs::copy(src, &dst) {
                    println!("cargo:warning=Failed to copy {}: {}", file, e);
                } else {
                    println!("cargo:warning=Copied {} to {:?}", file, dst);
                }
            }
        }

        // Copy tasks.toml
        let src = Path::new("tasks.toml");
        let dst = profile_dir.join("tasks.toml");

        if src.exists() {
            println!("cargo:rerun-if-changed=tasks.toml");
            if let Err(e) = fs::copy(src, &dst) {
                println!("cargo:warning=Failed to copy tasks.toml: {}", e);
            } else {
                println!("cargo:warning=Copied tasks.toml to {:?}", dst);
            }
        }
    }
}
