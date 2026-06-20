#!/usr/bin/env bash
# Rsync source to a remote CloudDesktop, build there, install the binary,
# install/refresh the systemd user unit, restart the service.
#
#   REMOTE=marvin ops/deploy.sh   (or just ops/deploy.sh — marvin is the default)

set -euo pipefail

REMOTE="${REMOTE:-marvin}"
SRC_DIR="$(cd "$(dirname "$0")/.." && pwd)"

echo ">> syncing source to $REMOTE"
rsync -az --delete \
  --exclude target \
  --exclude node_modules \
  --exclude 'frontend/dist' \
  --exclude .git \
  --exclude .claude \
  "$SRC_DIR/" "$REMOTE:tripit-src/"

echo ">> building on $REMOTE"
ssh "$REMOTE" 'bash -lc "set -euo pipefail
  cd ~/tripit-src
  make build
  install -D backend/target/release/tripit ~/.local/bin/tripit
  install -D ops/tripit.service ~/.config/systemd/user/tripit.service
  mkdir -p ~/.tripit
  systemctl --user daemon-reload
  systemctl --user enable --now tripit
  systemctl --user restart tripit
  loginctl enable-linger \$USER 2>/dev/null || true
  systemctl --user status tripit --no-pager | head -10
"'
echo ">> done"
