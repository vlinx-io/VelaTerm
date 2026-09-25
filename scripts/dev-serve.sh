#!/usr/bin/env bash
#
# dev-serve.sh [label] — plaintext development backend plus Vite HMR on random ports.
#
# Bind mode, selected with VLX_DEV_BIND:
#   lan (default, also when unset): Vite and the backend both bind 0.0.0.0 (`--lan-http`), so the same
#     instance serves this machine and anything else on the LAN: a second computer, a phone browser, or
#     the phone shell. A browser here still reaches it over localhost.
#   loopback: the backend runs with `--local-http` and Vite binds 127.0.0.1, so nothing listens on the
#     LAN. Use it on machines that must not expose ports to their network (a shared or always-on dev
#     machine reached only through SSH or a VPN such as Tailscale); reach it from your own machine with
#     `ssh -L <port>:localhost:<port> <this-host>`. HMR follows the page origin, so it works through that
#     tunnel. Do not publish it with `tailscale serve` or another shared proxy: with `--local-http` every
#     client gets the local management level, so everyone who can reach the proxy would. The banner shows
#     only the local URL and never prints the password; without VELA_SERVE_PASSWORD a random password is
#     generated and kept in <data dir>/dev-password (mode 0600) instead of the default `dev`.
#   Any other value stops the script before anything starts.
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
#   VLX_DEV_BIND=loopback pnpm dev:web    # Same, bound to 127.0.0.1 only (reach it through a tunnel)
#   pnpm dev:web uitest                   # Same, with an explicit label for dev:ls / dev:stop
#   VLX_DEV_DATA_DIR="$HOME/Library/Application Support/io.vlinx.vlxterm" pnpm dev:web
#                                         # Use the real database only when no other backend writes it
#   VELA_SERVE_PASSWORD=mypw pnpm dev:web # Custom login password
#
# VLX_DEV_DRY_RUN=1 resolves the configuration, prints the banner plus the resolved bindings, and exits
# without probing ports, registering the instance, or starting anything (used by dev-serve.test.mjs).
#
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"

MODE="web"

# Resolve the bind mode first so a typo fails before anything is registered or started.
DEV_BIND="${VLX_DEV_BIND:-lan}"
case "$DEV_BIND" in
  lan)
    HTTP_FLAG="--lan-http"
    PROBE_HOST="0.0.0.0" ;;
  loopback)
    HTTP_FLAG="--local-http"
    PROBE_HOST="127.0.0.1" ;;
  *)
    echo "✗ Unknown VLX_DEV_BIND value: '${DEV_BIND}' (accepted: loopback, lan; unset means lan)." >&2
    exit 2 ;;
esac
DRY_RUN="${VLX_DEV_DRY_RUN:-}"

# Register a label from environment, first argument, or automatic generation for dev:ls/dev:stop.
source "$(dirname "$0")/dev-lib.sh"
LABEL="$(dev_label "$MODE" "${1:-}")"

PASSWORD="${VELA_SERVE_PASSWORD:-dev}"
DATA_DIR="${VLX_DEV_DATA_DIR:-$ROOT/.dev-data/$MODE}"
mkdir -p "$DATA_DIR"

# Reserve ports free on the bind address: 0.0.0.0 in lan mode (so they work for loopback and LAN
# bindings), 127.0.0.1 in loopback mode. A dry run probes nothing and reports port 0.
free_port() { node -e 'const s=require("net").createServer();s.listen(0,process.argv[1],()=>{process.stdout.write(String(s.address().port));s.close()})' "$PROBE_HOST"; }
if [ -n "$DRY_RUN" ]; then
  BACKEND_PORT=0
  VITE_PORT=0
else
  BACKEND_PORT="$(free_port)"
  VITE_PORT="$(free_port)"
fi

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
LOCAL_URL="http://localhost:${VITE_PORT}"

if [ "$DEV_BIND" = "loopback" ]; then
  # Clients connect to Vite on 127.0.0.1 only. Leave the HMR host unset so the HMR socket follows the
  # page origin: localhost here, or whatever address a tunnel presents on the other machine.
  CLIENT_HOST="localhost"
  export VLX_VITE_HOST="127.0.0.1"
  unset VLX_VITE_HMR_HOST
  # An inherited TAURI_DEV_HOST (for example from a Tauri mobile dev shell) would make vite.config.ts start a
  # separate HMR socket on that address, which may be a LAN address; loopback mode must not listen there.
  unset TAURI_DEV_HOST
  URL="$LOCAL_URL"
else
  LAN_IP="$(lan_ip)"

  # Clients connect to Vite. Bind it to 0.0.0.0 and point HMR at the LAN address: a page loaded over the
  # LAN IP must reconnect its HMR socket to that same host, not to the server's own idea of localhost.
  CLIENT_HOST="$LAN_IP"
  export VLX_VITE_HOST="0.0.0.0"
  export VLX_VITE_HMR_HOST="$LAN_IP"
  URL="http://${CLIENT_HOST}:${VITE_PORT}"
fi

# Register the instance; cleanup removes it on exit. The bind mode rides along as an extra string field,
# which dev:ls / dev:stop ignore. A dry run registers nothing.
if [ -z "$DRY_RUN" ]; then
  dev_write_instance "$LABEL" "$MODE" \
    "vitePort=$VITE_PORT" "backendPort=$BACKEND_PORT" "dataDir=$DATA_DIR" "url=$URL" "bind=$DEV_BIND"
fi

if [ "$DEV_BIND" = "loopback" ]; then
  # Never print the password in loopback mode; name its source instead. Without VELA_SERVE_PASSWORD the
  # well-known default `dev` is replaced by a random password kept in a 0600 file of this instance's data
  # dir (reused on the next start), because a loopback instance runs with the local management level.
  if [ -n "${VELA_SERVE_PASSWORD:-}" ]; then
    PASSWORD_SOURCE="from VELA_SERVE_PASSWORD (not shown)"
  else
    PASSWORD_FILE="${DATA_DIR}/dev-password"
    PASSWORD_SOURCE="generated, stored in ${PASSWORD_FILE} (mode 0600, not shown)"
    if [ -z "$DRY_RUN" ]; then
      if [ ! -s "$PASSWORD_FILE" ]; then
        # node is already required above; a tr|head pipe would die of SIGPIPE under pipefail.
        ( umask 077; node -e 'process.stdout.write(require("crypto").randomBytes(12).toString("hex"))' > "$PASSWORD_FILE" )
      fi
      chmod 600 "$PASSWORD_FILE"
      PASSWORD="$(cat "$PASSWORD_FILE")"
    fi
  fi
  echo "============================================================"
  echo " dev · web mode (HMR)  ·  instance ${LABEL}  ·  loopback only"
  echo "   URL       : ${URL}      (bound to 127.0.0.1, not reachable from the network)"
  echo "   Remote    : ssh -L ${VITE_PORT}:localhost:${VITE_PORT} <this-host>  (not tailscale serve: local-mode rights)"
  echo "   Password  : ${PASSWORD_SOURCE}"
  echo "   Data dir  : ${DATA_DIR}"
  echo "   Ports     : Vite ${VITE_PORT} / backend ${BACKEND_PORT} (both random, so instances run in parallel)"
  echo " Ctrl+C stops both the backend and Vite; elsewhere, use pnpm dev:stop ${LABEL}"
  echo "============================================================"
else
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
fi

# Dry run: report the resolved bindings and stop before anything is started.
if [ -n "$DRY_RUN" ]; then
  echo "DRY_RUN bind=${DEV_BIND} httpFlag=${HTTP_FLAG} probeHost=${PROBE_HOST} viteHost=${VLX_VITE_HOST} hmrHost=${VLX_VITE_HMR_HOST:-} url=${URL}"
  exit 0
fi

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

# 3. Open a local browser once Vite answers. In lan mode other devices connect themselves using the LAN
#    URL above; in loopback mode they go through a tunnel.
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
