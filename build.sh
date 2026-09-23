#!/usr/bin/env bash
set -e
cd "$(dirname "${BASH_SOURCE[0]}")"

TARGETS=(x86_64-unknown-linux-gnu i686-unknown-linux-gnu x86_64-pc-windows-gnu i686-pc-windows-gnu)

rm -rf dist
mkdir -p dist

for triple in "${TARGETS[@]}"; do
  cargo build --release --target "$triple" --features steam2-cli/debug-tools --bin steam2extract_d
  cargo build --release --target "$triple" --bin steam2extract_f

  out="dist/$triple"
  mkdir -p "$out"
  src="target/$triple/release"
  ext=""
  [[ "$triple" == *windows* ]] && ext=".exe"

  cp "$src/steam2extract_f$ext" "$out/"
  cp "$src/steam2extract_d$ext" "$out/"
  cp -r "$src/bin" "$out/bin"
  cp NOTICE "$out/NOTICE"
  cp LICENSE "$out/LICENSE"

  {
    echo "Steam2ExtractV2 build log"
    echo "target: $triple"
    echo "date:   $(date -u +"%Y-%m-%d %H:%M:%S UTC")"
    echo
    echo "files:"
    find "$out" -type f -not -name "build.log" | sort | while read -r f; do
      printf "  %-40s %10d bytes\n" "${f#"$out"/}" "$(stat -c%s "$f")"
    done
  } > "$out/build.log"
done
