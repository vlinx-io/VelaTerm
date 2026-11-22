//! Public TypeScript SDK contract between VelaTerm's frontend host and external ESM plugins.

import type { ComponentType } from "react";
import type { Backdrop } from "../components/Backdrop";
import type { ContextMenu } from "../components/ContextMenu";
import type { Field, FieldStack } from "../components/Field";
import type { FormModal } from "../components/FormModal";
import type Icons from "../components/Icons";
import type { PasswordField } from "../components/PasswordField";
import type Select from "../components/Select";
import type { Locale } from "../i18n";

export interface VlxPluginUiKit {
  Select: typeof Select;
  Field: typeof Field;
  FieldStack: typeof FieldStack;
  PasswordField: typeof PasswordField;
  ContextMenu: typeof ContextMenu;
  Backdrop: typeof Backdrop;
  FormModal: typeof FormModal;
  Icons: typeof Icons;
}

export type PluginTranslateFn = (key: string, ...args: Array<string | number>) => string;

export interface VlxPluginHost {
  readonly pluginId: string;
  /** Invoke a backend RPC command on this plugin's sidecar process. */
  invoke: <T = unknown>(command: string, args?: Record<string, unknown>) => Promise<T>;
  /** Subscribe to an event emitted by this plugin's sidecar process. */
  listen: <T = unknown>(eventName: string, handler: (payload: T) => void) => () => void;
  /** Open a terminal tab backed by a virtual PTY shell URI (e.g. `colab://<name>`). */
  openTerminal: (opts: { shell: string; name?: string; title?: string }) => void;
  /** Open an external URL in the user's system browser. */
  openExternal: (url: string) => Promise<void>;
  /** Navigate this plugin's center page to a sub-route (e.g. `"overview"`, `"tasks"`). */
  navigate: (subRoute: string | null) => void;
  /** Read the current sub-route of this plugin (or null when inactive). */
  useRoute: () => string | null;
  /** Translate a plugin-scoped i18n key reactively as the user changes interface language. */
  useT: () => PluginTranslateFn;
  /** Current interface locale code (e.g. `"en"`, `"zh-CN"`). */
  getLocale: () => Locale;
  /** Shared Vlinx UI components so plugins match the native interface. */
  readonly ui: VlxPluginUiKit;
}

export interface VlxPluginRegistration {
  /** Component rendered inside LeftSidebar when the plugin's rail icon is selected. */
  Sidebar?: ComponentType;
  /** Component rendered inside CenterPane when `?plugin=<id>` is active. */
  CenterPage?: ComponentType<{ route: string }>;
  /** Optional cleanup hook invoked before unloading or reloading the plugin. */
  dispose?: () => void;
}

export interface VlxPluginModule {
  activate: (host: VlxPluginHost) => VlxPluginRegistration | Promise<VlxPluginRegistration>;
}
