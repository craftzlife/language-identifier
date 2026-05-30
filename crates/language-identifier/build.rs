// Build-script for the `llm-apple-foundation` feature.
//
// Compiles `src/llm/bridge/LanguageModelBridge.swift` into a static
// library and emits the link directives the Rust crate needs to consume
// it (the Swift static lib + `FoundationModels.framework` + Apple's
// Swift runtime stubs). When the feature is off — or the target OS
// isn't macOS — this script is a no-op so the crate still builds on
// Linux / Windows for the non-LLM features.

use std::env;
use std::path::PathBuf;
use std::process::Command;

const SWIFT_SRC: &str = "src/layers/llm/bridge/LanguageModelBridge.swift";
// macOS 26 is the first release that ships the `FoundationModels`
// framework with `SystemLanguageModel` / `LanguageModelSession`. Bumping
// this requires a matching bump in `Cargo.toml`'s feature doc-comment.
const MIN_MACOS: &str = "26.0";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={SWIFT_SRC}");

    let feature_enabled = env::var("CARGO_FEATURE_LLM_APPLE_FOUNDATION").is_ok();
    if !feature_enabled {
        return;
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "macos" {
        // Match the `#[cfg(target_os = "macos")]` gate in `llm/mod.rs`:
        // the Rust side compiles away to nothing on non-Apple targets,
        // so we don't need a static lib either. Skipping (rather than
        // erroring) lets a workspace consumer enable the feature
        // globally and still cross-build for Linux.
        return;
    }

    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let swift_target = match arch.as_str() {
        "aarch64" => format!("arm64-apple-macos{MIN_MACOS}"),
        "x86_64" => format!("x86_64-apple-macos{MIN_MACOS}"),
        other => panic!(
            "llm-apple-foundation: unsupported macOS target arch `{other}` \
             (expected aarch64 or x86_64)"
        ),
    };

    let sdk_path = macos_sdk_path()
        .expect("xcrun --sdk macosx --show-sdk-path must succeed for llm-apple-foundation");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));
    let object_path = out_dir.join("LanguageModelBridge.o");
    let lib_path = out_dir.join("libLanguageModelBridge.a");

    let swiftc = which_swiftc();

    // Step 1: compile the Swift file to an object file. We don't use
    // `swiftc -emit-library -static` directly because cargo prefers
    // driving `ar`/`libtool` itself, and a single .o keeps the link
    // line simple. `-parse-as-library` lets the file omit a top-level
    // `main`.
    let compile = Command::new(&swiftc)
        .args(["-target", &swift_target, "-sdk"])
        .arg(&sdk_path)
        .args([
            "-O",
            "-parse-as-library",
            "-emit-object",
            "-module-name",
            "LanguageModelBridge",
            "-o",
        ])
        .arg(&object_path)
        .arg(SWIFT_SRC)
        .status()
        .expect("failed to spawn swiftc");
    assert!(compile.success(), "swiftc failed to compile {SWIFT_SRC}");

    // Step 2: pack the object into a static archive cargo can link.
    let ar_status = Command::new("ar")
        .args(["crus"])
        .arg(&lib_path)
        .arg(&object_path)
        .status()
        .expect("failed to spawn ar");
    assert!(ar_status.success(), "ar failed to archive {object_path:?}");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=LanguageModelBridge");

    // Apple system frameworks the bridge uses directly.
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=FoundationModels");

    // The Swift runtime symbols the .o references. On modern macOS the
    // libs themselves are in the dyld shared cache, but the link step
    // still needs a search path to resolve the references at compile
    // time — point it at the active toolchain.
    if let Some(swift_libdir) = swift_runtime_libdir() {
        println!("cargo:rustc-link-search=native={}", swift_libdir.display());
    }
    println!("cargo:rustc-link-lib=dylib=swiftCore");
    println!("cargo:rustc-link-lib=dylib=swift_Concurrency");

    // The Swift runtime dylibs have install names like
    // `@rpath/libswift_Concurrency.dylib`. On macOS the OS-shipped
    // copies live in the dyld cache under `/usr/lib/swift/`, so we
    // need to add that as a runtime search path or dyld can't find
    // them when the test binary launches.
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
}

fn which_swiftc() -> PathBuf {
    // Prefer the Xcode-selected toolchain via `xcrun -f swiftc`. Falls
    // back to a bare `swiftc` on PATH for CI images that don't ship
    // xcrun but do have the Swift toolchain installed elsewhere.
    if let Ok(out) = Command::new("xcrun").args(["-f", "swiftc"]).output() {
        if out.status.success() {
            let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !path.is_empty() {
                return PathBuf::from(path);
            }
        }
    }
    PathBuf::from("swiftc")
}

fn swift_runtime_libdir() -> Option<PathBuf> {
    // Mirrors swiftc's own runtime resolution: the Swift stdlib + runtime
    // libs live under the active toolchain at `usr/lib/swift/macosx`.
    let out = Command::new("xcrun")
        .args(["--find", "swiftc"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let swiftc = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    // swiftc path: <toolchain>/usr/bin/swiftc → <toolchain>/usr/lib/swift/macosx
    let toolchain_bin = swiftc.parent()?; // .../usr/bin
    let toolchain_usr = toolchain_bin.parent()?; // .../usr
    Some(toolchain_usr.join("lib").join("swift").join("macosx"))
}

fn macos_sdk_path() -> Option<PathBuf> {
    let out = Command::new("xcrun")
        .args(["--sdk", "macosx", "--show-sdk-path"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(PathBuf::from(s))
    }
}
