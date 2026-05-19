#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
cd "$repo_root"

version="${VERSION:-}"
if [[ -z "$version" ]]; then
    version="$(sed -nE 's/^version[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' Cargo.toml | head -n 1)"
fi
if [[ -z "$version" ]]; then
    echo "Could not read package version from Cargo.toml" >&2
    exit 1
fi

dist="$repo_root/dist"
name="cptool-v${version}-linux-x86_64"
package_dir="$dist/$name"
archive="$dist/$name.tar.gz"
target_dir="$repo_root/target/release-linux"

mkdir -p "$dist"
rm -rf "$package_dir" "$archive"

CARGO_TARGET_DIR="$target_dir" cargo build --release

mkdir -p "$package_dir"
cp "$target_dir/release/cptool" "$package_dir/cptool"
chmod 755 "$package_dir/cptool"
cp "$repo_root/README.md" "$package_dir/README.md"

tar -C "$dist" -czf "$archive" "$name"
"$package_dir/cptool" --version
echo "created $archive"
