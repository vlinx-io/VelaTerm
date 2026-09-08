/// <reference types="vite/client" />

/** Application version injected at build time; see define in vite.config.ts. */
declare const __APP_VERSION__: string;

/** Build timestamp injected by the release script in YYYYMMDD-HHmm format; empty in development/ordinary builds. */
declare const __BUILD_TIME__: string;

/** Development-server build flag injected at build time when `pnpm dev:server` sets VLX_DEV_BUILD=1. When true,
 *  the title bar displays the same DEV badge as a development run, using the build timestamp, making it easy to
 *  distinguish from a release. Release-script and ordinary builds always set this to false. */
declare const __DEV_BUILD__: boolean;

declare module "turndown-plugin-gfm" {
  import type TurndownService from "turndown";

  export const gfm: TurndownService.Plugin;
}
