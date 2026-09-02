#!/usr/bin/env bash
#
# dev-serve.sh [label] — plaintext development backend plus Vite HMR on random ports.
#
# Vite and the backend both bind 0.0.0.0 (`--lan-http`), so the same instance serves this machine and
# anything else on the LAN: a second computer, a phone browser, or the phone shell. There is no
# separate loopback-only variant; a browser here still reaches it over localhost.
#
# Clients connect to Vite, which proxies /ws and /api to the real backend while preserving HMR.
#
# Design:
# 1. Random Vite/backend ports allow Tauri, Electron, and several web instances to run concurrently.
# 2. Defaults to `.dev-data/web` for a safe parallel database that starts with an empty tree.
# 3. Password defaults to `dev` and may be overridden with VELA_SERVE_PASSWORD.
#
# Plaintext over the LAN is a development-only setting: `--lan-http` refuses to start on a release
# build (any identifier ending in `.release`), so this script cannot expose a production binary.
#
# Usage:
#   pnpm dev:web                          # Browser with HMR and an isolated empty database
#   pnpm dev:web uitest                   # Same, with an explicit label for dev:ls / dev:stop
#   VLX_DEV_DATA_DIR="$HOME/Library/Application Support/io.vlinx.vlxterm" pnpm dev:web
#                                         # Use the real database only when no other backend writes it
#   VELA_SERVE_PASSWORD=mypw pnpm dev:web # Custom login password
#
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"

MODE="web"
HTTP_FLAG="--lan-http"

# Register a label from environment, first argument, or automatic generation for dev:ls/dev:stop.
source "$(dirname "$0")/dev-lib.sh"
LABEL="$(dev_label "$MODE" "${1:-}")"

PASSWORD="${VELA_SERVE_PASSWORD:-dev}"
DATA_DIR="${VLX_DEV_DATA_DIR:-$ROOT/.dev-data/$MODE}"
mkdir -p "$DATA_DIR"

# Reserve ports free on 0.0.0.0 so they work for loopback and LAN bindings.
free_port() { node -e 'const s=require("net").createServer();s.listen(0,"0.0.0.0",()=>{process.stdout.write(String(s.address().port));s.close()})'; }
BACKEND_PORT="$(free_port)"
VITE_PORT="$(free_port)"

# Discover the first non-loopback IPv4 using platform-specific methods, so the printed URL is one that
# other machines can also open. Fall back to 127.0.0.1 nonfatally when discovery fails.
lan_ip() {
  case "$(uname -s)" in
    Darwin)
      ipconfig getifaddr en0 2>/dev/null || ipconfig getifaddr en1 2>/dev/null || echo 127.0.0.1 ;;
    Linux)
      { hostname -I 2>/dev/null | awk '{print $1}'; } | grep -E '.' || echo 127.0.0.1 ;;
    MINGW*|MSYS*|CYGWIN*)
      ipconfig 2>/dev/null | grep -a 'IPv4' | grep -aoE '[0-9]+(\.[0-9]+){3}' \
        | grep -vE '^127\.' | head -1 | grep -E '.' || echo 127.0.0.1 ;;
    *) echo 127.0.0.1 ;;
  esac
}
LAN_IP="$(lan_ip)"

# Clients connect to Vite. Bind it to 0.0.0.0 and point HMR at the LAN address: a page loaded over the
# LAN IP must reconnect its HMR socket to that same host, not to the server's own idea of localhost.
CLIENT_HOST="$LAN_IP"
export VLX_VITE_HOST="0.0.0.0"
export VLX_VITE_HMR_HOST="$LAN_IP"
URL="http://${CLIENT_HOST}:${VITE_PORT}"
LOCAL_URL="http://localhost:${VITE_PORT}"

# Register the instance; cleanup removes it on exit.
dev_write_instance "$LABEL" "$MODE" \
  "vitePort=$VITE_PORT" "backendPort=$BACKEND_PORT" "dataDir=$DATA_DIR" "url=$URL"

echo "============================================================"
echo " dev · web mode (HMR)  ·  instance ${LABEL}"
echo "   URL       : ${URL}      (also reachable from other devices on this network)"
echo "   Local     : ${LOCAL_URL}"
echo "   Password  : ${PASSWORD}"
echo "   Data dir  : ${DATA_DIR}"
echo "   Ports     : Vite ${VITE_PORT} / backend ${BACKEND_PORT} (both random, so instances run in parallel)"
echo "   Pair link : vlxterm://pair?host=${CLIENT_HOST}&port=${VITE_PORT}&password=${PASSWORD}&name=dev"
echo "   (type the link into Add host in the phone app, or turn it into a QR code)"
echo " Ctrl+C stops both the backend and Vite; elsewhere, use pnpm dev:stop ${LABEL}"
echo "============================================================"

# On exit or interruption, terminate backend/Vite children and remove the registry entry.
pids=()
cleanup() { trap - INT TERM EXIT; for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done; dev_remove_instance; }
trap cleanup INT TERM EXIT

# 1. Start the debug backend on a random plaintext port and isolated database. Pass password by environment.
VELA_SERVE_PASSWORD="$PASSWORD" \
  cargo run --manifest-path src-tauri/Cargo.toml -- \
    --serve "$HTTP_FLAG" --port "$BACKEND_PORT" --data-dir "$DATA_DIR" &
pids+=($!)

# 2. Start Vite HMR on a random port and proxy /ws and /api to the backend.
VLX_VITE_PORT="$VITE_PORT" VLX_DEV_BACKEND="http://127.0.0.1:${BACKEND_PORT}" \
  pnpm exec vite &
pids+=($!)

# 3. Open a local browser once Vite answers. Other devices connect themselves using the LAN URL above.
(
  for _ in $(seq 1 60); do
    if curl -s -o /dev/null "http://127.0.0.1:${VITE_PORT}/" 2>/dev/null; then
      open "$LOCAL_URL" 2>/dev/null \
        || xdg-open "$LOCAL_URL" 2>/dev/null \
        || powershell.exe -NoProfile -Command "Start-Process '$LOCAL_URL'" 2>/dev/null \
        || cmd.exe //c start "" "$LOCAL_URL" 2>/dev/null \
        || true
      break
    fi
    sleep 0.5
  done
) &

wait
