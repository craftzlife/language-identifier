// Cargo's `rustc-link-arg` directive from a dependency's build script
// doesn't propagate to dependents, so downstream binary crates that
// link `language-identifier` with the `llm-apple-foundation` feature
// need to re-emit the Swift runtime rpath themselves. The CLI crate
// does that here.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let feature = std::env::var("CARGO_FEATURE_LLM_APPLE_FOUNDATION").is_ok();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if feature && target_os == "macos" {
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    }
}
