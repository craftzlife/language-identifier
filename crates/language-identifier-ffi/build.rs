//! Generates `language_identifier.h` from the `lid_*` extern "C"
//! functions in `src/c_abi.rs`. The header lands in
//! `<workspace-root>/target/c-header/language_identifier.h` so build
//! scripts can pick it up alongside the platform `.so` / `.dll`.

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/c_abi.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");

    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    // Workspace target dir = $CARGO_MANIFEST_DIR/../../target.
    // Falls back to OUT_DIR if the layout ever changes.
    let header_dir = PathBuf::from(&crate_dir)
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("target").join("c-header"))
        .unwrap_or_else(|| PathBuf::from(env::var("OUT_DIR").unwrap()));

    std::fs::create_dir_all(&header_dir).expect("create c-header dir");
    let out = header_dir.join("language_identifier.h");

    let config = cbindgen::Config::from_file(format!("{crate_dir}/cbindgen.toml"))
        .expect("read cbindgen.toml");

    match cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config)
        .generate()
    {
        Ok(bindings) => {
            bindings.write_to_file(&out);
            println!("cargo:warning=cbindgen → {}", out.display());
        }
        Err(e) => {
            // Don't fail the build on header-gen errors (esp. during
            // cross-compiles where the parser may stumble on cfg'd
            // items). The library itself is what matters.
            println!("cargo:warning=cbindgen skipped: {e}");
        }
    }
}
