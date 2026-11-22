//! Exposes the host's single React runtime and Vlinx UI kit on `globalThis.__VLX_SHARED__`
//! so external ESM plugin bundles share the exact same React instance (avoiding duplicate-hook
//! errors) and native Vlinx components without bundling copies of React.

import * as React from "react";
import * as ReactDOM from "react-dom";
import * as ReactDOMClient from "react-dom/client";
import * as ReactJsxRuntime from "react/jsx-runtime";
import { Backdrop } from "../components/Backdrop";
import { ContextMenu } from "../components/ContextMenu";
import { Field, FieldStack } from "../components/Field";
import { FormModal } from "../components/FormModal";
import Icons from "../components/Icons";
import { PasswordField } from "../components/PasswordField";
import Select from "../components/Select";
import type { VlxPluginUiKit } from "./sdk";

export interface VlxSharedRuntime {
  react: typeof React;
  reactDom: typeof ReactDOM;
  reactDomClient: typeof ReactDOMClient;
  reactJsxRuntime: typeof ReactJsxRuntime;
  ui: VlxPluginUiKit;
}

const SHARED_UI: VlxPluginUiKit = {
  Select,
  Field,
  FieldStack,
  PasswordField,
  ContextMenu,
  Backdrop,
  FormModal,
  Icons,
};

let installed = false;

export function getPluginUiKit(): VlxPluginUiKit {
  return SHARED_UI;
}

export function initPluginSharedGlobals(): void {
  if (installed || typeof globalThis === "undefined") return;
  installed = true;
  (globalThis as unknown as { __VLX_SHARED__?: VlxSharedRuntime }).__VLX_SHARED__ = {
    react: React,
    reactDom: ReactDOM,
    reactDomClient: ReactDOMClient,
    reactJsxRuntime: ReactJsxRuntime,
    ui: SHARED_UI,
  };
}
