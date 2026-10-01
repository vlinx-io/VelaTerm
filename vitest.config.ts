import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Headless component-test configuration (jsdom). This is separate from Tauri's vite.config.ts and only injects
// build-time constants and enables the jsdom environment.
export default defineConfig({
  plugins: [react()],
  define: {
    __APP_VERSION__: JSON.stringify("test"),
    __BUILD_TIME__: JSON.stringify(""),
  },
  test: {
    environment: "jsdom",
    globals: true,
    // scripts/*.test.mjs covers the dev launcher scripts; those files opt into the node environment themselves.
    include: ["src/**/*.test.{ts,tsx}", "scripts/**/*.test.mjs"],
    // Work around Node 22+'s experimental global localStorage masking jsdom's implementation; see that file.
    setupFiles: ["./vitest.setup.ts"],
  },
});
