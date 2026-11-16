#!/bin/sh
set -eu

fail() {
  echo "tntop: $*" >&2
  exit 1
}

case "$(uname -s)/$(uname -m)" in
Darwin/arm64 | Darwin/aarch64) target=aarch64-apple-darwin ;;
Linux/x86_64) target=x86_64-unknown-linux-gnu ;;
*) fail "Unsupported platform: $(uname -s)/$(uname -m)" ;;
esac

version=${1:-latest}
directory=${2:-"$HOME/.local/bin"}
base=https://aaronmallen.dev/aaron/tntop/releases/download
if [ "$version" = latest ]; then
  version=$(curl --proto '=https' --tlsv1.2 -fsSL https://aaronmallen.dev/api/v1/repos/aaron/tntop/releases/latest |
    sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
fi
printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || fail "Invalid release version: $version"
asset=tntop-$version-$target.tar.gz
temporary=$(mktemp -d)
staged=
trap 'rm -rf "$temporary"; [ -z "$staged" ] || rm -f "$staged"' EXIT
trap 'exit 1' HUP INT TERM
curl --proto '=https' --tlsv1.2 -fsSL "$base/$version/$asset" -o "$temporary/$asset"
curl --proto '=https' --tlsv1.2 -fsSL "$base/$version/SHA256SUMS" -o "$temporary/SHA256SUMS"
expected=$(awk -v asset="$asset" '$2 == asset || $2 == "*" asset {print $1}' "$temporary/SHA256SUMS")
[ "${#expected}" -eq 64 ] || fail "Missing or duplicate checksum for $asset"
case "$expected" in *[!0-9a-fA-F]*) fail "Invalid checksum for $asset" ;; esac
if command -v sha256sum >/dev/null 2>&1; then
  actual=$(sha256sum "$temporary/$asset" | awk '{print $1}')
else
  actual=$(shasum -a 256 "$temporary/$asset" | awk '{print $1}')
fi
[ "$actual" = "$expected" ] || fail "Checksum mismatch for $asset"
tar -xzf "$temporary/$asset" -C "$temporary" --strip-components=1 "${asset%.tar.gz}/tntop"
[ -f "$temporary/tntop" ] && [ ! -L "$temporary/tntop" ] || fail "Archive has no regular tntop binary"
mkdir -p "$directory"
staged=$(mktemp "$directory/.tntop.XXXXXX")
cp "$temporary/tntop" "$staged"
chmod 755 "$staged"
mv -f "$staged" "$directory/tntop"
staged=
echo "Installed tntop $version to $directory/tntop"
case ":$PATH:" in
*":$directory:"*) ;;
*)
  echo "Add $directory to PATH."
  echo "Bash/Zsh: add export PATH=\"$directory:\$PATH\" to your shell configuration."
  echo "Fish: run fish_add_path \"$directory\"."
  ;;
esac
