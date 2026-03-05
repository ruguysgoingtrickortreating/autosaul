use std::env;
use std::path::PathBuf;

fn main() {
    #[cfg(target_os = "windows")]
    {
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let natives_dir = manifest_dir.join("lib");

        println!("cargo:rustc-link-search=native={}", natives_dir.display());
    }
}