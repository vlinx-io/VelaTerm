import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// Vite dev-server port: 1420 by default. dev:tauri, dev:web, and dev:electron each pass a random
// available port through VLX_VITE_PORT, allowing multiple modes and worktrees to run concurrently without collisions.
// @ts-expect-error process is a nodejs global
const vitePort = Number(process.env.VLX_VITE_PORT) || 1420;

// Host binding: dev:web sets VLX_VITE_HOST=0.0.0.0 so other devices on the LAN can connect. Otherwise, retain
// TAURI_DEV_HOST for Tauri testing on a physical device; if neither is set, listen only on localhost.
// @ts-expect-error process is a nodejs global
const bindHost = process.env.VLX_VITE_HOST || host || false;

// HMR callback host: when a client such as a phone loads the page over a LAN IP, the HMR client must connect
// back to this host:vitePort. Its default page origin may be unreachable through a LAN/proxy. Mobile mode sets
// VLX_VITE_HMR_HOST to the LAN IP.
// @ts-expect-error process is a nodejs global
const hmrHost = process.env.VLX_VITE_HMR_HOST;

// Backend used for browser UI debugging; /ws and /api are proxied here. dev:web points
// VLX_DEV_BACKEND at their own randomly assigned plaintext backend. Port 8799 remains the default for the
// legacy manual workflow.
// @ts-expect-error process is a nodejs global
const devBackend = process.env.VLX_DEV_BACKEND || "https://localhost:8799";

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],

  // Inject the version at build time from package.json through pnpm's npm_package_version, along with the
  // build timestamp passed by release.sh in VLX_BUILD_TIME (format: YYYYMMDD-HHmm). Ordinary development and
  // builds not launched through the script use an empty timestamp.
  define: {
    __APP_VERSION__: JSON.stringify(process.env.npm_package_version || "0.0.0"),
    __BUILD_TIME__: JSON.stringify(process.env.VLX_BUILD_TIME || ""),
    // Development server build marker: pnpm dev:server sets VLX_DEV_BUILD=1 to display the DEV badge in the title bar.
    __DEV_BUILD__: JSON.stringify(process.env.VLX_DEV_BUILD === "1"),
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: vitePort,
    strictPort: true,
    host: bindHost,
    hmr: hmrHost
      ? {
          // Mobile: the phone loads the page over a LAN IP, and the HMR client reconnects to the same host:vitePort.
          protocol: "ws",
          host: hmrHost,
          clientPort: vitePort,
        }
      : host
        ? {
            // Preserve the original TAURI_DEV_HOST behavior for physical-device Tauri testing (dedicated port 1421).
            protocol: "ws",
            host,
            port: 1421,
          }
        : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
    // 4. Browser UI debugging proxy: forward WebSocket traffic on /ws and HTTP traffic on /api to devBackend.
    //    The backend uses self-signed HTTPS, so secure:false accepts its certificate; /ws needs ws:true to
    //    forward protocol upgrades. The login cookie does not use Secure (see src-tauri/src/web/auth.rs), so
    //    http://localhost:1420 can retain the session. Desktop `pnpm tauri dev` is unaffected because its WebView
    //    uses Tauri IPC and never touches these routes.
    proxy: {
      "/ws": {
        target: devBackend,
        ws: true,
        secure: false,
        changeOrigin: true,
      },
      "/api": {
        target: devBackend,
        secure: false,
        changeOrigin: true,
      },
    },
  },
}));
