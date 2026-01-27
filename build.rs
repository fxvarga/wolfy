use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Process LALRPOP grammar
    lalrpop::process_root().unwrap();

    // Copy default.rasi to the output directory
    let out_dir = env::var("OUT_DIR").unwrap();
    // OUT_DIR is something like target/x86_64-pc-windows-gnu/release/build/wolfy-xxx/out
    // We need to go up to the target profile directory (release or debug)
    let out_path = Path::new(&out_dir);

    // Navigate up: out -> wolfy-xxx -> build -> release -> target-triple -> target
    // We want: target/x86_64-pc-windows-gnu/release/
    if let Some(profile_dir) = out_path.ancestors().nth(3)
    // up 3 levels from OUT_DIR
    {
        let src = Path::new("default.rasi");
        let dst = profile_dir.join("default.rasi");

        if src.exists() {
            println!("cargo:rerun-if-changed=default.rasi");
            if let Err(e) = fs::copy(src, &dst) {
                println!("cargo:warning=Failed to copy default.rasi: {}", e);
            } else {
                println!("cargo:warning=Copied default.rasi to {:?}", dst);
            }
        }
    }
}
