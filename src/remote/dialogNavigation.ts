//! Stable URL navigation for the project forms and their shared folder picker.

import { useCallback, useEffect, useState } from "react";

export type ProjectDialog = "clone" | "create" | "open" | "save";
const DIALOGS: ProjectDialog[] = ["clone", "create", "open", "save"];
const DRAFT_KEYS = ["projectParent", "projectName", "projectBranch", "projectCollectionId", "picker", "pickerPath", "pickerFilter", "pickerSelected", "pickerHidden", "saveName"];
let lockedDialog: { kind: ProjectDialog; href: string } | null = null;

export function readProjectDialog(): ProjectDialog | null {
  const value = new URLSearchParams(window.location.search).get("projectDialog");
  return DIALOGS.includes(value as ProjectDialog) ? value as ProjectDialog : null;
}

export function readProjectDialogCollection(): string | null {
  return new URLSearchParams(window.location.search).get("projectCollectionId");
}

export function projectDialogUrl(kind: ProjectDialog | null, collectionId?: string | null): string {
  const url = new URL(window.location.href);
  if (kind !== readProjectDialog()) DRAFT_KEYS.forEach(key => url.searchParams.delete(key));
  if (kind) {
    url.searchParams.set("projectDialog", kind);
    if (collectionId !== undefined) {
      if (collectionId) url.searchParams.set("projectCollectionId", collectionId);
      else url.searchParams.delete("projectCollectionId");
    }
  }
  else {
    url.searchParams.delete("projectDialog");
    DRAFT_KEYS.forEach(key => url.searchParams.delete(key));
  }
  return url.href;
}

export function navigateProjectDialog(kind: ProjectDialog, open: boolean, replace = false, collectionId?: string | null) {
  if (!open && readProjectDialog() !== kind) return;
  const href = projectDialogUrl(open ? kind : null, collectionId);
  if (href === window.location.href) return;
  window.history[replace ? "replaceState" : "pushState"](null, "", href);
  window.dispatchEvent(new PopStateEvent("popstate"));
}

// A repository URL can carry credentials, so a prefilled one stays in memory instead of the address bar.
let clonePreset = "";
/** Prefills the clone form's repository URL the next time it opens. */
export function presetCloneUrl(url: string) { clonePreset = url; }
/** Returns and clears the prefilled repository URL. */
export function takeCloneUrl(): string { const url = clonePreset; clonePreset = ""; return url; }

export function writeDialogDraft(key: string, value: string) {
  const url = new URL(window.location.href);
  if (!readProjectDialog()) return;
  if (value) url.searchParams.set(key, value); else url.searchParams.delete(key);
  window.history.replaceState(null, "", url);
}

/** Recover presentation state only; opening a URL never starts a filesystem mutation. */
export function useDialogDraft(key: string, active: boolean, fallback = ""): [string, (value: string) => void] {
  const read = useCallback(() => new URLSearchParams(window.location.search).get(key) ?? fallback, [key, fallback]);
  const [value, setValue] = useState(read);
  useEffect(() => {
    if (!active) return;
    setValue(read());
    const update = () => setValue(read());
    window.addEventListener("popstate", update);
    return () => window.removeEventListener("popstate", update);
  }, [active, read]);
  const update = useCallback((next: string) => {
    setValue(next);
    writeDialogDraft(key, next);
  }, [key]);
  return [value, update];
}

/** A running mutation must retain its progress/cancel surface when Back is pressed. */
export function useDialogNavigationLock(kind: ProjectDialog, busy: boolean) {
  useEffect(() => {
    if (!busy) return;
    lockedDialog = { kind, href: window.location.href };
    return () => { if (lockedDialog?.kind === kind) lockedDialog = null; };
  }, [kind, busy]);
  return useCallback(() => { if (lockedDialog?.kind === kind) lockedDialog = null; }, [kind]);
}

export function restoreLockedDialog(): boolean {
  if (!lockedDialog || window.location.href === lockedDialog.href) return false;
  window.history.replaceState(null, "", lockedDialog.href);
  return true;
}

export function setLocationPickerOpen(open: boolean) {
  const url = new URL(window.location.href);
  if (open) url.searchParams.set("picker", "location");
  else ["picker", "pickerPath", "pickerFilter", "pickerSelected", "pickerHidden"].forEach(key => url.searchParams.delete(key));
  if (url.href === window.location.href) return;
  window.history.pushState(null, "", url);
  window.dispatchEvent(new PopStateEvent("popstate"));
}
