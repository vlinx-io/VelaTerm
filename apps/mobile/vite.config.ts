import { defineConfig } from "vite";
import { readFileSync } from "node:fs";

const { version } = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8"));
// Fixed at build time so reopening the app shows the same value, which identifies the installed package.
// Stored as UTC ISO text; the app formats it in the device's locale and time zone.
const builtAt = new Date().toISOString();
export default defineConfig({
  define: {
    __MOBILE_VERSION__: JSON.stringify(version),
    __MOBILE_BUILD_TIME__: JSON.stringify(builtAt),
  },
  server: { port: 41571, strictPort: true },
  preview: { port: 41571, strictPort: true },
});
