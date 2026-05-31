#!/usr/bin/env bash
# Build Android .so libs for all 4 ABIs and generate the Kotlin binding.
#
# Prerequisites (one-time):
#   rustup target add aarch64-linux-android armv7-linux-androideabi \
#                     x86_64-linux-android i686-linux-android
#   cargo install cargo-ndk
#   Install the Android NDK and set ANDROID_NDK_HOME (or ANDROID_NDK_ROOT).
#
# Output layout (under target/android/):
#   jniLibs/
#     arm64-v8a/liblanguage_identifier_ffi.so
#     armeabi-v7a/liblanguage_identifier_ffi.so
#     x86_64/liblanguage_identifier_ffi.so
#     x86/liblanguage_identifier_ffi.so
#   kotlin/io/languageidentifier/language_identifier_ffi.kt
#
# Drop jniLibs/ under src/main/ of your Android module (or pack into an
# AAR). Copy the .kt into the same module's source set.
set -euo pipefail

cd "$(dirname "$0")/../../.."  # repo root

CRATE=language-identifier-ffi
OUT=target/android

rm -rf "$OUT"
mkdir -p "$OUT/jniLibs"

echo "==> Building .so libs for all 4 Android ABIs"
cargo ndk \
  -t arm64-v8a \
  -t armeabi-v7a \
  -t x86_64 \
  -t x86 \
  -o "$OUT/jniLibs" \
  build --release -p "$CRATE"

# cargo-ndk writes into jniLibs/<abi>/lib...so. The Kotlin binding's
# System.loadLibrary("language_identifier_ffi") then resolves at runtime
# via the platform's per-ABI selection.

echo "==> Generating Kotlin binding"
# Bindgen needs a host build to introspect — use a release dylib from
# the default host triple. If no host build exists yet, kick one off.
HOST_DYLIB_DARWIN="target/release/liblanguage_identifier_ffi.dylib"
HOST_LIB_LINUX="target/release/liblanguage_identifier_ffi.so"
if [[ ! -f "$HOST_DYLIB_DARWIN" && ! -f "$HOST_LIB_LINUX" ]]; then
  cargo build --release -p "$CRATE"
fi
HOST_LIB="$HOST_DYLIB_DARWIN"
[[ -f "$HOST_LIB" ]] || HOST_LIB="$HOST_LIB_LINUX"

cargo run --release -p "$CRATE" --bin uniffi-bindgen -- \
  generate \
  --library "$HOST_LIB" \
  --language kotlin \
  --out-dir "$OUT/kotlin" \
  --config crates/language-identifier-ffi/uniffi.toml

echo
echo "Done. Artifacts:"
echo "  $OUT/jniLibs/  ← copy under your app module's src/main/"
echo "  $OUT/kotlin/io/languageidentifier/language_identifier_ffi.kt"
echo
echo "Gradle: add 'net.java.dev.jna:jna:5.14.0@aar' as a dependency."
echo "(JNA is UniFFI's transport layer on the Kotlin/JVM side.)"
