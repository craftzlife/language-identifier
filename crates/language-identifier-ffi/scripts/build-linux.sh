#!/usr/bin/env bash
# Build Linux .so for x86_64 + aarch64 and copy the C header.
# Consumers `#include "language_identifier.h"` and link against the
# .so; the three `lid_*` functions return JSON-encoded `IdentifyResult`.
#
# Prerequisites (one-time):
#
#   # Native build on Linux:
#   rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
#
#   # Cross-compile from macOS: easiest is `cross` (Docker-based):
#   cargo install cross
#   # Then set CROSS=1 below.
#
# Output (target/linux/):
#   x86_64/liblanguage_identifier_ffi.so
#   aarch64/liblanguage_identifier_ffi.so
#   include/language_identifier.h
set -euo pipefail

cd "$(dirname "$0")/../../.."

CRATE=language-identifier-ffi
OUT=target/linux
BUILDER="cargo"
[[ "${CROSS:-0}" == "1" ]] && BUILDER="cross"

rm -rf "$OUT"
mkdir -p "$OUT/x86_64" "$OUT/aarch64" "$OUT/include"

echo "==> Building .so for x86_64 + aarch64 (builder: $BUILDER)"
for triple in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu; do
  $BUILDER build --release --target "$triple" -p "$CRATE"
done

cp target/x86_64-unknown-linux-gnu/release/liblanguage_identifier_ffi.so "$OUT/x86_64/"
cp target/aarch64-unknown-linux-gnu/release/liblanguage_identifier_ffi.so "$OUT/aarch64/"

cp target/c-header/language_identifier.h "$OUT/include/"

echo
echo "Done. Artifacts:"
echo "  $OUT/x86_64/liblanguage_identifier_ffi.so"
echo "  $OUT/aarch64/liblanguage_identifier_ffi.so"
echo "  $OUT/include/language_identifier.h"
echo
echo "C++ consumption:"
echo "  #include \"language_identifier.h\""
echo "  char* json = lid_identify(\"some text\");"
echo "  /* parse json with nlohmann::json or rapidjson, etc. */"
echo "  lid_string_free(json);"
echo
echo "Link:  -L target/linux/x86_64 -llanguage_identifier_ffi"
echo "       (set rpath/LD_LIBRARY_PATH as appropriate for distribution)"
