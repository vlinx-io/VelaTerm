#!/usr/bin/env bash
#
# dev-desktop.sh — Tauri desktop development with random Vite ports for parallel instances/worktrees.
#
# This is fundamentally `pnpm tauri dev`: launch a Tauri desktop window whose WebView uses Tauri IPC and no
# network port, while automatically starting Vite for frontend HMR.
#
# Tauri's devUrl is static configuration (default http://localhost:1420), so multiple Tauri instances/worktrees
# would collide on 1420. Allocate one free port and provide it to both:
#   1. Vite through VLX_VITE_PORT, read by vite.config.ts with 1420 still the default.
#   2. Tauri by overriding devUrl through `--config` with the same port.
# Matching the two lets every launch use an independent random port without collisions.
#
# Instance label: the first positional argument not starting with `-`, or VLX_DEV_LABEL. Forward remaining
# arguments unchanged to `tauri dev`. dev:ls / dev:stop use the label for precise management (see dev-lib.sh).
#
# This previously used `exec pnpm tauri dev`, replacing the script process. It now starts in the background and
# uses wait + trap so the instance registry can be created/removed. Ctrl+C and window-close behavior are unchanged.
#
set -euo pipefail

cd "$(dirname "$0")/.."
source "$(dirname "$0")/dev-lib.sh"

# ── First-run preparation inherited from dev.cmd: install missing dependencies. On Windows, fetch the minimal
#    Git Bash resource if absent or Terminal sessions fall back to PowerShell (see agent/gitbash.rs). macOS/Linux
#    do not need Git Bash.
[ -d node_modules ] || { echo "==> node_modules is missing, running pnpm install first"; pnpm install; }
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    if [ ! -f src-tauri/resources/gitbash/usr/bin/bash.exe ]; then
      if [ -f scripts/fetch-gitbash.ps1 ]; then
        echo "==> resources/gitbash is empty, fetching the minimal Git Bash first (fetch-gitbash.ps1)"
        powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/fetch-gitbash.ps1 \
          || { echo "✗ fetch-gitbash failed, aborting" >&2; exit 1; }
      else
        # The bundled Git Bash is a packaging input, not a build input: the dev window runs fine without it.
        # Aborting here makes `pnpm dev:desktop` unusable on Windows whenever the fetch script is unavailable,
        # so degrade to a warning and let the developer decide.
        echo "==> scripts/fetch-gitbash.ps1 not found; continuing without the bundled Git Bash." >&2
        echo "    Windows' bundled default-shell option will be unavailable in this dev build." >&2
      fi
    fi ;;
esac

# Parse the label: if the first argument is not a Tauri flag (does not start with -), consume it as the label.
LABEL_CAND=""
if [ "${1:-}" ] && [ "${1#-}" = "$1" ]; then
  LABEL_CAND="$1"
  shift
fi
LABEL="$(dev_label tauri "$LABEL_CAND")"

# Ask the kernel for a free port.
PORT="$(node -e 'const s=require("net").createServer();s.listen(0,"127.0.0.1",()=>{process.stdout.write(String(s.address().port));s.close()})')"

echo "==> Tauri dev: Vite port ${PORT} (random, so instances run in parallel)  ·  instance ${LABEL}. Ctrl+C stops it, or run pnpm dev:stop ${LABEL}."

# Register the running instance. Tauri has no network backend port, so record only the Vite port.
dev_write_instance "$LABEL" tauri "vitePort=$PORT"

# On exit/interruption, terminate the Tauri process tree and remove its registry entry.
cleanup() { trap - INT TERM EXIT; [ -n "${TAURI_PID:-}" ] && kill "$TAURI_PID" 2>/dev/null || true; dev_remove_instance; }
trap cleanup INT TERM EXIT

# VLX_VITE_PORT configures Vite launched by beforeDevCommand; --config points Tauri devUrl to the same port.
# VLX_BUILD_TIME puts the build time in the title-bar badge, matching former dev.cmd behavior; vite.config.ts
# injects __BUILD_TIME__.
export VLX_VITE_PORT="$PORT"
export VLX_BUILD_TIME="$(date '+%Y%m%d-%H%M')"
pnpm tauri dev --config "{\"build\":{\"devUrl\":\"http://localhost:${PORT}\"}}" "$@" &
TAURI_PID=$!
wait "$TAURI_PID"
