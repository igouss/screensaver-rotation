#!/bin/bash

# Install screensaver-rotation from this checkout: build the binary into
# ~/.local/bin, make the runners directory, and link the Omarchy launcher
# override into place. Screensavers join by linking themselves into the
# runners directory from their own install.sh.

set -euo pipefail

repo=$(cd "$(dirname "$0")" && pwd)
override="$HOME/.local/share/omarchy-overrides/bin/omarchy-launch-screensaver"

cargo build --release --locked --manifest-path "$repo/Cargo.toml"
# install(1) replaces the file rather than rewriting it, so a running
# launcher keeps its copy.
install -Dm755 "$repo/target/release/screensaver-rotation" "$HOME/.local/bin/screensaver-rotation"
echo "built ~/.local/bin/screensaver-rotation"

mkdir -p "$HOME/.local/lib/screensaver-rotation/runners"

mkdir -p "$(dirname "$override")"
if [[ -e $override && ! -L $override ]]; then
  mv "$override" "$override.orig"
  echo "kept the previous $override as $override.orig"
fi
ln -sfn "$repo/omarchy/omarchy-launch-screensaver" "$override"
echo "linked $override -> $repo/omarchy/omarchy-launch-screensaver"

if ! grep -qs 'omarchy-overrides/bin' "$HOME/.bashrc"; then
  cat <<'MSG'

One manual step: Omarchy's idle service starts the screensaver through `bash -lc`,
so the override directory must come first on PATH. Add this to ~/.bashrc right
after the line that sources Omarchy's env-bootstrap:

  [[ ":$PATH:" == *":$HOME/.local/share/omarchy-overrides/bin:"* ]] || PATH="$HOME/.local/share/omarchy-overrides/bin:$PATH"
MSG
fi
