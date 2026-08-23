//! Automatic updates for the desktop Tauri client.
//!
//! `check()` fetches the updater endpoint configured under `plugins.updater` in tauri.conf.json and
//! compares it with the installed version, returning an Update for a newer release or null otherwise.
//! The client performs this check directly; no separate server API is required.
//!
//! UX: discovering a release never interrupts the user with a modal. The silent startup check adds an
//! indicator to the status bar; clicking it opens UpdateModal for release notes and installation.
//! An explicit Check for Updates menu action opens the modal immediately. Downloads continue in the
//! background after the modal closes, with progress shown in the status bar.
//!
//! The native `ask()` from `@tauri-apps/plugin-dialog` is unsuitable: two buttons leave no room for
//! Skip This Version, release notes, or download progress. Silent multi-megabyte downloads after a
//! simple Yes prompt otherwise make the application appear frozen.
//!
//! State lives in the small store below. Only this module writes it, while the status bar and modal
//! read it, keeping updater concerns out of termStore.
//!
//! Browser and remote clients lack the updater plugin and do not self-update, so `!isTauri` skips it.
//!
//! Every check carries the `X-Install-Id` header, an anonymous identifier the backend generates once per
//! installation. It lets the update server count installations instead of IP addresses, which merge users
//! behind one NAT and split a single user whose address changes. Checks repeat on a schedule as well as at
//! startup, because a terminal often stays open for days and would otherwise never report again.

import { message } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { useSyncExternalStore } from "react";

import { LOCALES, t, type Locale } from "../i18n";
import { invoke, isTauri } from "./transport";
import { localizeReleaseNotes, sliceReleaseNotes } from "./updateNotes";

/** Version recorded by Skip This Version. It affects only silent startup checks; explicit menu checks
 * ignore it so users can still install a previously skipped release. */
const SKIPPED_KEY = "vlx-skipped-version";

/** Read the skipped version, treating unavailable localStorage as no skipped version. */
export function loadSkippedVersion(): string | null {
  try {
    return localStorage.getItem(SKIPPED_KEY);
  } catch {
    return null;
  }
}

function saveSkippedVersion(version: string) {
  try {
    localStorage.setItem(SKIPPED_KEY, version);
  } catch {
    /* Ignore unavailable localStorage; at worst the release is offered again next launch. */
  }
}

/** Clear the skipped version. Explicit menu checks already bypass it and need not call this. */
export function clearSkippedVersion() {
  try {
    localStorage.removeItem(SKIPPED_KEY);
  } catch {
    /* Same fallback as above. */
  }
}

/** Immutable information for one update prompt, fixed once a new release is found. */
export interface UpdatePrompt {
  /** Plugin Update handle used for download and installation; close it when dismissing the prompt. */
  update: Update;
  version: string;
  currentVersion: string;
  /** Changelog Markdown sliced after the installed version; empty when no notes apply. */
  notes: string;
  /** Localized release notes, with untranslated release sections falling back to English. */
  localizedNotes: Partial<Record<Locale, string>>;
  /** Direct platform installer URL from the endpoint, offered when automatic updating fails. */
  downloadUrl: string | null;
}

/** Current update phase, used by both the status bar and modal to select their presentation. */
export type UpdateStage =
  | { kind: "available" }
  | { kind: "downloading"; received: number; total: number }
  | { kind: "installing" }
  /** Installed and awaiting restart. Windows never reaches this because the plugin exits after install. */
  | { kind: "ready" }
  | { kind: "error"; detail: string };

export interface UpdateState {
  /** null means there is no pending release, either none was found or it was skipped. */
  prompt: UpdatePrompt | null;
  stage: UpdateStage;
  /** Whether the modal is open; it is a detail view while the status-bar indicator persists. */
  modalOpen: boolean;
}

const IDLE: UpdateState = {
  prompt: null,
  stage: { kind: "available" },
  modalOpen: false,
};

let state: UpdateState = IDLE;
const listeners = new Set<() => void>();

function setState(patch: Partial<UpdateState>) {
  state = { ...state, ...patch };
  for (const fn of listeners) fn();
}

function subscribe(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

/** Subscribe to shared update state; closing the modal does not affect a background download. */
export function useUpdateState(): UpdateState {
  return useSyncExternalStore(
    subscribe,
    () => state,
    () => IDLE,
  );
}

/** Open the update modal from the status-bar indicator or an explicit menu check. */
export function openUpdateModal() {
  if (state.prompt) setState({ modalOpen: true });
}

/** Close the modal while retaining the status indicator and any active download. */
export function closeUpdateModal() {
  setState({ modalOpen: false });
}

/** Whether download or installation is active, when dismissing the indicator would be misleading. */
function isBusy(): boolean {
  return state.stage.kind === "downloading" || state.stage.kind === "installing";
}

/**
 * Dismiss the current update prompt and its status-bar indicator.
 * @param skip When true, remember this release so silent startup checks ignore it until a newer
 *   version appears or the user explicitly selects Check for Updates.
 */
export function dismissUpdate({ skip = false }: { skip?: boolean } = {}) {
  const current = state.prompt;
  if (!current || isBusy()) return;
  if (skip) saveSkippedVersion(current.version);
  setState({ ...IDLE });
  void current.update.close().catch(() => {});
}

/** Download and install in the background, publishing progress to the status bar through the store. */
export async function startInstall(): Promise<void> {
  const current = state.prompt;
  if (!current || isBusy()) return;
  setState({ stage: { kind: "downloading", received: 0, total: 0 } });
  try {
    let received = 0;
    let total = 0;
    await current.update.downloadAndInstall((ev) => {
      if (ev.event === "Started") {
        total = ev.data.contentLength ?? 0;
        received = 0;
        setState({ stage: { kind: "downloading", received, total } });
      } else if (ev.event === "Progress") {
        received += ev.data.chunkLength;
        setState({ stage: { kind: "downloading", received, total } });
      } else if (ev.event === "Finished") {
        setState({ stage: { kind: "installing" } });
      }
    });
    // Windows never reaches this: the plugin launches the installer with ShellExecute and exits.
    setState({ stage: { kind: "ready" } });
  } catch (err) {
    console.error("[updater] download or installation failed", err);
    setState({ stage: { kind: "error", detail: String(err) } });
  }
}

/** Restart the application after installation on macOS or Linux. */
export async function restartApp(): Promise<void> {
  await relaunch();
}

/** Reentrancy guard for repeated menu clicks or overlapping silent and explicit checks. */
let checking = false;

/** Read localized changelogs from the manifest's `notes_i18n` extension. Tauri preserves unknown
 * manifest fields in rawJson, allowing older clients to use standard notes and newer ones to localize. */
function localizedNotesFromManifest(
  rawJson: Record<string, unknown>,
  notes: string,
  currentVersion: string,
): Partial<Record<Locale, string>> {
  const raw = rawJson.notes_i18n;
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return {};

  const result: Partial<Record<Locale, string>> = {};
  for (const locale of LOCALES) {
    if (locale === "en") continue;
    const translated = (raw as Record<string, unknown>)[locale];
    if (typeof translated !== "string" || !translated.trim()) continue;
    const merged = localizeReleaseNotes(notes, translated, currentVersion);
    if (merged) result[locale] = merged;
  }
  return result;
}

/** Cached installation identifier. The backend issues it once and never changes it, so one read per
 * process is enough; a failed read stays null and simply retries on the next check. */
let cachedInstallId: string | null = null;

/** Anonymous installation identifier for update-check telemetry, or null when it cannot be read.
 * Failure is never fatal: the check proceeds without the header and the server records the row anyway. */
async function installId(): Promise<string | null> {
  if (cachedInstallId) return cachedInstallId;
  try {
    const id = await invoke<string>("install_id");
    cachedInstallId = id || null;
  } catch (err) {
    console.error("[updater] could not read install id", err);
    cachedInstallId = null;
  }
  return cachedInstallId;
}

/** Call the updater endpoint with the installation header attached. */
async function checkWithId(): Promise<Update | null> {
  const id = await installId();
  return check(id ? { headers: { "X-Install-Id": id } } : undefined);
}

/**
 * Repeat the request purely for its server-side record and release the handle.
 *
 * A pending prompt makes `checkForUpdates` return early, which would silence telemetry for exactly the
 * installations that have not updated yet — the ones whose count matters most. This keeps them visible
 * without touching the prompt the user has already been shown.
 */
async function pingForTelemetry(): Promise<void> {
  try {
    const update = await checkWithId();
    if (update) await update.close().catch(() => {});
  } catch (err) {
    console.error("[updater] telemetry-only update check failed", err);
  }
}

/**
 * Check for updates.
 * @param manual True for an explicit Check for Updates action: report no-update and failure results,
 *   ignore the skipped-version record, and open the modal immediately. False for startup checks:
 *   show only an unskipped new release in the status bar, with no modal or error interruption.
 */
export async function checkForUpdates({
  manual = false,
}: { manual?: boolean } = {}): Promise<boolean> {
  if (!isTauri) return false;
  // Reuse an existing pending prompt. An explicit check opens it rather than replacing its Update
  // handle with a second one and leaking the first. This runs before the in-flight guard: reopening a
  // dialog needs no network, and making the menu item silently do nothing while a background check
  // happens to be running is worse than reopening it.
  if (state.prompt && manual) {
    openUpdateModal();
    return false;
  }
  if (checking) return false;
  if (state.prompt) {
    checking = true;
    try {
      await pingForTelemetry();
    } finally {
      checking = false;
    }
    return true;
  }
  checking = true;
  try {
    const update = await checkWithId();
    if (!update) {
      if (manual) {
        await message(t("updater.upToDate"), { title: t("updater.title") });
      }
      return true;
    }
    if (!manual && loadSkippedVersion() === update.version) {
      await update.close().catch(() => {});
      return true;
    }
    const url = update.rawJson.url;
    const notes = update.body ?? "";
    setState({
      prompt: {
        update,
        version: update.version,
        currentVersion: update.currentVersion,
        // update.body contains the full latest.json changelog; retain only releases newer than local.
        notes: sliceReleaseNotes(notes, update.currentVersion),
        localizedNotes: localizedNotesFromManifest(
          update.rawJson,
          notes,
          update.currentVersion,
        ),
        downloadUrl: typeof url === "string" ? url : null,
      },
      stage: { kind: "available" },
      // Silent checks only light the status bar; explicit checks open the requested details.
      modalOpen: manual,
    });
  } catch (err) {
    console.error("[updater] update check failed", err);
    if (manual) {
      await message(t("updater.failed", String(err)), {
        title: t("updater.title"),
        kind: "error",
      });
    }
  } finally {
    checking = false;
  }
  return true;
}

/** Delay before the first check so it never competes with startup work for the main thread. */
const STARTUP_DELAY_MS = 5_000;

/** Gap between silent checks. A terminal commonly stays open for days, so a startup-only check would
 * both miss releases published mid-session and, on the server side, make long-running installations
 * invisible in usage counts. */
const CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;

/** How often the schedule compares the clock against the interval above. */
const TICK_MS = 5 * 60 * 1000;

/**
 * Start silent update checks: one shortly after launch, then one every six hours.
 *
 * The interval is enforced against the wall clock rather than by a six-hour timer, because a laptop
 * that sleeps stops firing timers: on wake the next tick sees the elapsed time and checks immediately,
 * instead of waiting out a timer that stood still. Returns a function that stops the schedule.
 */
export function startUpdateSchedule(): () => void {
  if (!isTauri) return () => {};
  let lastCheckAt = Date.now();
  const run = () => {
    // Advance the clock only when a check actually ran. A scheduled run that collided with a manual
    // check returns early, and resetting the clock for it would postpone the next silent check by a
    // full interval; leaving the clock alone lets the next tick retry within minutes.
    const startedAt = Date.now();
    void checkForUpdates({ manual: false }).then((ran) => {
      if (ran) lastCheckAt = startedAt;
    });
  };
  const startup = setTimeout(run, STARTUP_DELAY_MS);
  const tick = setInterval(() => {
    if (Date.now() - lastCheckAt >= CHECK_INTERVAL_MS) run();
  }, TICK_MS);
  return () => {
    clearTimeout(startup);
    clearInterval(tick);
  };
}
