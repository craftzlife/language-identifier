#!/usr/bin/env bash
# Build an XCFramework + Swift bindings for macOS (arm64 + x86_64) and
# iOS (device arm64, simulator arm64 + x86_64).
#
# Prerequisites (one-time):
#   rustup target add aarch64-apple-darwin x86_64-apple-darwin \
#                     aarch64-apple-ios \
#                     aarch64-apple-ios-sim x86_64-apple-ios
#
# Output layout (under target/apple/):
#   LanguageIdentifier.xcframework/   ← drop into an Xcode project or SwiftPM
#   swift/                             ← generated Swift sources (Sources/)
set -euo pipefail

cd "$(dirname "$0")/../../.."  # repo root

CRATE=language-identifier-ffi
LIB=liblanguage_identifier_ffi.a
OUT=target/apple

rm -rf "$OUT"
mkdir -p "$OUT"

echo "==> Building static libs for all Apple targets"
for triple in \
  aarch64-apple-darwin x86_64-apple-darwin \
  aarch64-apple-ios \
  aarch64-apple-ios-sim x86_64-apple-ios
do
  cargo build --release --target "$triple" -p "$CRATE"
done

echo "==> Lipo'ing macOS + iOS-sim universal slices"
lipo -create \
  "target/aarch64-apple-darwin/release/$LIB" \
  "target/x86_64-apple-darwin/release/$LIB" \
  -output "$OUT/macos-$LIB"

lipo -create \
  "target/aarch64-apple-ios-sim/release/$LIB" \
  "target/x86_64-apple-ios/release/$LIB" \
  -output "$OUT/iossim-$LIB"

echo "==> Generating Swift bindings"
cargo run --release -p "$CRATE" --bin uniffi-bindgen -- \
  generate \
  --library "target/aarch64-apple-darwin/release/$LIB" \
  --language swift \
  --out-dir "$OUT/swift" \
  --config crates/language-identifier-ffi/uniffi.toml

# XCFramework wants headers in their own directory, with module.modulemap
# named exactly that. Pivot the generated layout.
HEADERS="$OUT/headers"
mkdir -p "$HEADERS"
mv "$OUT/swift/LanguageIdentifierFFI.h" "$HEADERS/"
mv "$OUT/swift/LanguageIdentifierFFI.modulemap" "$HEADERS/module.modulemap"

echo "==> Assembling XCFramework"
xcodebuild -create-xcframework \
  -library "$OUT/macos-$LIB" -headers "$HEADERS" \
  -library "$OUT/iossim-$LIB" -headers "$HEADERS" \
  -library "target/aarch64-apple-ios/release/$LIB" -headers "$HEADERS" \
  -output "$OUT/LanguageIdentifier.xcframework"

echo
echo "Done. Artifacts:"
echo "  $OUT/LanguageIdentifier.xcframework  ← bundle this in your app"
echo "  $OUT/swift/LanguageIdentifier.swift  ← add to Sources/"
echo
echo "Swift Package consumers: copy the .xcframework + .swift into your"
echo "Package.swift's .binaryTarget(...) / .target(...) layout."
