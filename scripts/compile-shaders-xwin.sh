#!/usr/bin/env bash
# Linux → x86_64-pc-windows-msvc helper: gpui-pre-windows only compiles its
# HLSL shaders when the *build host* is Windows (its build.rs gates on
# cfg(target_os = "windows")). Under cargo-xwin that step never runs and the
# crate's include!(OUT_DIR/shaders_bytes.rs) fails. This script reproduces the
# exact output the crate's build.rs emits — same fxc invocations, same
# `const NAME: &[u8] = &[...]` bindings — into the crate's OUT_DIR.
#
# Requires: fxc.exe reachable via wine (GPUI_FXC_PATH pointing at fxc.exe or a
# wrapper that forwards args). On a real Windows host this script is
# unnecessary — cargo build handles it natively.
#
# Usage:
#   GPUI_FXC_PATH=/path/to/fxc-or-wrapper \
#     scripts/compile-shaders-xwin.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
# memoria-gpui consumes gpui-pre-windows from crates.io (no vendor/); the
# newest extracted copy in the registry is the one cargo builds.
src="$(ls -dt "${CARGO_HOME:-$HOME/.cargo}"/registry/src/*/gpui-pre-windows-* 2>/dev/null | head -1)"
[ -n "$src" ] || { echo "gpui-pre-windows not found in the cargo registry — run cargo fetch first" >&2; exit 1; }

out_dir="$(ls -dt "$root"/target/x86_64-pc-windows-msvc/release/build/gpui-pre-windows-*/out 2>/dev/null | head -1)"
if [ -z "$out_dir" ]; then
  # Force the (no-op on Linux) build script to run so OUT_DIR exists.
  cargo build --release --locked --target x86_64-pc-windows-msvc \
    --manifest-path "$root/Cargo.toml" --target-dir "$root/target" 2>/dev/null || true
  out_dir="$(ls -dt "$root"/target/x86_64-pc-windows-msvc/release/build/gpui-pre-windows-*/out | head -1)"
fi
[ -d "$out_dir" ] || { echo "could not locate gpui-pre-windows OUT_DIR" >&2; exit 1; }

fxc="${GPUI_FXC_PATH:?set GPUI_FXC_PATH to fxc.exe or a wine wrapper}"
binding="$out_dir/shaders_bytes.rs"
: > "$binding"

compile() { # module shader_hlsl
  local module="$1" hlsl="$2" upper
  upper="$(echo "$module" | tr '[:lower:]' '[:upper:]')"
  for kind in vertex:vs fragment:ps; do
    local entry="${module}_${kind%%:*}" profile="${kind##*:}_4_1"
    local header="$out_dir/${module}_${kind##*:}.h" name="${upper}_$([ "${kind%%:*}" = vertex ] && echo VERTEX || echo FRAGMENT)_BYTES"
    "$fxc" /T "$profile" /E "$entry" /Fh "$header" /Vn "$name" /O3 "$hlsl" >/dev/null
    python3 - "$header" "$name" >> "$binding" <<'PY'
import re, sys
header, name = sys.argv[1], sys.argv[2]
body = open(header).read()
arr = body[body.index("const BYTE"):]
arr = arr[arr.index("=") + 1:].strip()
print(f"const {name}: &[u8] = &{arr.replace('{', '[').replace('}', ']')}")
PY
  done
}

for m in quad shadow path_rasterization path_sprite underline monochrome_sprite subpixel_sprite polychrome_sprite; do
  compile "$m" "$src/src/shaders.hlsl"
done
compile emoji_rasterization "$src/src/color_text_raster.hlsl"
echo "wrote $binding"
