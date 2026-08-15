//! Platform adapter entry point.
//!
//! Select the implementation at runtime and export the `platform` singleton and `env` environment view.
//! Application code consistently uses `import { platform } from "../platform"` (or another appropriate relative
//! path) instead of touching `@tauri-apps/*` / `electron` directly. ESLint's no-restricted-imports enforces this
//! rule, with platform/ itself exempt.

import { env } from "./env";
import { electronPlatform } from "./electron";
import { tauriPlatform } from "./tauri";
import type { Platform } from "./types";

/** Select at runtime: the Electron shell uses the Electron implementation; desktop Tauri and browser WS use the Tauri implementation. */
export const platform: Platform = env.isElectron ? electronPlatform : tauriPlatform;

export { env };

export type {
  BadgeCapability,
  BrowserCapability,
  BrowserRect,
  BrowserStatePayload,
  ClipboardCapability,
  DialogCapability,
  NotifyCapability,
  NotifyPermission,
  OpenerCapability,
  Platform,
  PlatformEnv,
  PlatformKind,
  QuitCapability,
  SaveFileOptions,
  TransportCapability,
  UnlistenFn,
  VelaCommandCapability,
  VelaCommandStatus,
  WindowCapability,
} from "./types";
