//! What each `vapp` action says on its confirmation card, and how the app window performs the actions the
//! host does not run. Card text is built only from validated fields, never from the agent's prose.

import type { AssistAction } from "../../ipc/assist";
import type { I18nKey, useT } from "../../i18n";
import { setLang, type LangChoice } from "../../i18n";
import en from "../../i18n/locales/en";
import { useTermStore } from "../../store/termStore";
import { genId } from "../../genId";
import { revealPath } from "../../ipc/info";
import { memoryUrl } from "../Memory/navigation";
import { notebookRoute, openVaultFile } from "../Notebook/shared";
import { presetCloneUrl, projectDialogUrl } from "../../remote/dialogNavigation";
import { sidebarTabOfView, sidebarTabSection } from "../LeftSidebar/sidebarTreeLayout";
import { SYSTEM_PROJECTS } from "../LeftSidebar/systemCollections";
import { agentPickerUrl } from "../NewAgentSession/navigation";
import { translationUrl } from "../../translation/navigation";
import type { AccentChoice, Density, Theme } from "../../theme";

type T = ReturnType<typeof useT>;
/** A key without parameters, used to type keys built at runtime. */
type Plain = "assist.confirm";

/** Translates a key built at runtime, falling back when the dictionary has no such entry. */
export function label(t: T, key: string, fallback = ""): string { return key in en ? t(key as Plain) : fallback || key; }
export interface ActionCard { title: string; details: string[]; opensForm?: boolean; danger?: boolean }

const str = (value: unknown) => typeof value === "string" ? value : "";
/** Field values as "label: value" lines, skipping empty ones. */
const lines = (t: T, pairs: [I18nKey, unknown][]) => pairs.filter(([, v]) => v !== undefined && v !== null && v !== "").map(([key, v]) => `${t(key as Plain)}: ${String(v)}`);

/** Settings page names, the same labels the settings window shows. */
const SETTINGS_PAGES: Record<string, I18nKey> = {
  general: "settings.catGeneral", appearance: "settings.appearance", terminal: "settings.catTerminal", conversation: "settings.chatTypography", translation: "translation.title",
  behavior: "settings.catBehavior", advanced: "settings.catAdvanced", agents: "settings.catAgents", shortcuts: "settings.catShortcuts",
};
const SETTING_VALUES: Record<string, I18nKey> = {
  "theme.system": "assist.value.system", "theme.dark": "assist.value.dark", "theme.light": "assist.value.light",
  "density.compact": "assist.value.compact", "density.regular": "assist.value.regular", "density.comfy": "assist.value.comfy",
  "accent.auto": "assist.value.auto", "accent.green": "assist.value.green", "accent.blue": "assist.value.blue", "accent.amber": "assist.value.amber", "accent.violet": "assist.value.violet",
};
/** Product names stay untranslated; only the plain terminal has a translated name. */
const SESSION_NAMES: Record<string, string> = {
  claude: "Claude", codex: "Codex", opencode: "OpenCode", copilot: "Copilot", cursor: "Cursor", antigravity: "Antigravity", cline: "Cline", pi: "Pi", omp: "OMP",
  crush: "Crush", kimi: "Kimi", kiro: "Kiro", grok: "Grok", zoo: "Zoo", zcode: "ZCode", deepseek: "DeepSeek",
};
const LANGUAGE_NAMES: Record<string, string> = { en: "English", "zh-CN": "简体中文", "zh-TW": "繁體中文", ja: "日本語", ko: "한국어", fr: "Français", de: "Deutsch", es: "Español", "pt-BR": "Português (Brasil)", ru: "Русский", vi: "Tiếng Việt" };

function settingValue(t: T, setting: string, value: unknown) {
  if (typeof value === "boolean") return t(value ? "assist.on" : "assist.off");
  if (setting === "language") return LANGUAGE_NAMES[str(value)] || t("assist.value.auto");
  const key = SETTING_VALUES[`${setting}.${str(value)}`];
  return key ? t(key as Plain) : str(value);
}

/** A collection's sidebar location; the built-in Projects collection is shown by its translated name. */
function collectionName(t: T, id: unknown, name: unknown): string {
  if (!id) return "";
  return id === SYSTEM_PROJECTS ? t("systemCollection.projects") : str(name);
}

/** A readable sidebar location the host built from the tree, such as `Course / Lessons`. */
const place = (where: unknown) => where ? [str(where)] : [];

export function describeAction(action: AssistAction, t: T): ActionCard {
  const a = action as Record<string, unknown> & AssistAction;
  const collection = collectionName(t, a.collectionId, a.collectionName);
  const into = collection ? [t("assist.projects.intoCollection", collection)] : [];
  switch (`${a.domain}.${a.kind}`) {
    case "projects.importProject": {
      const folders = Array.isArray(a.candidates) ? a.candidates.map(str) : [];
      // With one folder the card names it; with several the user picks one on the card.
      return { title: folders.length === 1 ? t("assist.projects.importFolder", folders[0]) : t("assist.projects.importProject"), details: into };
    }
    case "projects.createProject": return { title: t("assist.projects.createProject", str(a.name)), details: [...lines(t, [["assist.field.location", a.parentDir]]), ...into], opensForm: true };
    case "projects.cloneProject": return { title: t("assist.projects.cloneProject", str(a.url)), details: lines(t, [["assist.field.location", a.parentDir], ["assist.field.folder", a.folderName], ["assist.field.branch", a.branch]]), opensForm: true };
    case "projects.createCollection": {
      const parent = collectionName(t, a.parentId, a.parentName);
      return { title: t("assist.projects.createCollection", str(a.name)), details: parent ? [t("assist.projects.insideCollection", parent)] : [] };
    }
    case "projects.moveToCollection": return { title: t("assist.projects.moveToCollection", str(a.projectName), collection || t("systemCollection.projects")), details: [] };
    case "projects.createGroup": return { title: t("assist.projects.createGroup", str(a.name), str(a.where)), details: [] };
    case "projects.newSession": return { title: t("assist.projects.newSession", SESSION_NAMES[str(a.sessionKind)] ?? t("assist.terminal"), str(a.projectName)), details: a.groupName ? [t("assist.projects.insideGroup", str(a.groupName))] : [], opensForm: a.sessionKind !== "terminal" };
    case "projects.openSession": return { title: t("assist.projects.openSession", str(a.sessionName)), details: place(a.where) };
    case "projects.revealProject": return { title: t("assist.projects.revealProject", str(a.projectName)), details: [] };
    case "projects.rename": return { title: t("assist.projects.rename", str(a.currentName), str(a.name)), details: a.where !== a.currentName ? place(a.where) : [] };
    case "projects.move": return { title: t("assist.projects.move", str(a.nodeName), str(a.to)), details: [] };
    case "projects.sort": return { title: t(a.by === "order" ? "assist.projects.sortByOrder" : "assist.projects.sortByName", str(a.where)), details: [] };
    case "projects.killProcess": return { title: t("assist.projects.killProcess", str(a.nodeName)), details: [...place(a.where), t("assist.projects.killProcessNote")], danger: true };
    case "projects.archiveSession": return { title: t("assist.projects.archiveSession", str(a.nodeName)), details: place(a.where) };
    case "projects.deleteNode": return { title: t("assist.projects.deleteNode", str(a.nodeName)), details: [...place(a.where), t("assist.projects.deleteKeepsFiles")], danger: true };

    case "files.openFile": return { title: t("assist.files.openFile", str(a.path)), details: [] };
    case "files.reveal": return { title: t("assist.files.reveal", str(a.path)), details: [] };

    case "knowledge.openVault": return { title: t("assist.knowledge.openVault", str(a.vaultName)), details: [] };
    case "knowledge.search": return { title: t("assist.knowledge.search", str(a.query)), details: [a.allVaults ? t("assist.knowledge.allVaults") : str(a.vaultName)] };
    case "knowledge.openNote": return { title: t("assist.knowledge.openNote", str(a.path)), details: [str(a.vaultName)] };
    case "knowledge.createNote": return { title: t("assist.knowledge.createNote", str(a.vaultName)), details: lines(t, [["assist.field.name", a.name], ["assist.field.folder", a.folder]]), opensForm: true };
    case "knowledge.createFolder": return { title: t("assist.knowledge.createFolder", str(a.vaultName)), details: lines(t, [["assist.field.name", a.name], ["assist.field.folder", a.folder]]), opensForm: true };
    case "knowledge.moveNote": return { title: t("assist.knowledge.moveNote", str(a.path)), details: [str(a.vaultName)], opensForm: true };
    case "knowledge.deleteNote": return { title: t("assist.knowledge.deleteNote", str(a.path)), details: [str(a.vaultName)], opensForm: true };
    case "knowledge.favorite": return { title: t(a.favorite ? "assist.knowledge.favoriteOn" : "assist.knowledge.favoriteOff", str(a.path)), details: [str(a.vaultName)] };
    case "knowledge.importFiles": return { title: t("assist.knowledge.importFiles", str(a.vaultName)), details: lines(t, [["assist.field.folder", a.folder]]), opensForm: true };
    case "knowledge.registerVault": return { title: t("assist.knowledge.registerVault", str(a.path)), details: [] };
    case "knowledge.createVault": return { title: t("assist.knowledge.createVault"), details: [], opensForm: true };
    case "knowledge.openCollections": return { title: t("assist.knowledge.openCollections"), details: [] };

    case "settings.openSettings": return { title: t("assist.settings.openSettings"), details: [t((SETTINGS_PAGES[str(a.page)] ?? "settings.catGeneral") as Plain)] };
    case "settings.set": return { title: t("assist.settings.set", label(t, `assist.settings.name.${str(a.setting)}`), settingValue(t, str(a.setting), a.value)), details: [] };
    case "settings.setFontSize": return { title: t("assist.settings.setFontSize", label(t, `assist.settings.target.${str(a.target)}`), Number(a.size)), details: [] };
    case "settings.setFontFamily": return { title: a.family ? t("assist.settings.setFontFamily", label(t, `assist.settings.target.${str(a.target)}`), str(a.family)) : t("assist.settings.resetFontFamily", label(t, `assist.settings.target.${str(a.target)}`)), details: [] };
  }
  return { title: `${a.domain} · ${a.kind}`, details: [] };
}

/** Opens a page or form through its own URL, the same way its regular entry point does. */
function go(href: string) {
  if (href === window.location.href) return;
  window.history.pushState(window.history.state, "", href);
  window.dispatchEvent(new PopStateEvent("popstate"));
}
function withParams(href: string, values: Record<string, string | null | undefined>) {
  const url = new URL(href);
  for (const [key, value] of Object.entries(values)) if (value) url.searchParams.set(key, value); else url.searchParams.delete(key);
  return url.href;
}
/** Shows the Projects section of the sidebar without collapsing it when it is already shown. */
function showProjects() {
  const s = useTermStore.getState();
  if (!s.projects.some(p => p.id === SYSTEM_PROJECTS)) return;
  const shown = sidebarTabSection(sidebarTabOfView(s.sidebarTreeTabs, s.activeSidebarTreeViewId) ?? sidebarTabOfView(s.sidebarTreeTabs, s.primarySidebarTreeViewId));
  if (shown !== "projects" || s.leftCollapsed) s.pickSidebarSection("projects");
}

/** Performs an action the app window owns. Form actions open the app's own form, which does the work on submit. */
export async function performAction(action: AssistAction): Promise<void> {
  const a = action as Record<string, unknown> & AssistAction;
  const store = useTermStore.getState();
  switch (`${a.domain}.${a.kind}`) {
    case "projects.createProject":
      go(withParams(projectDialogUrl("create", str(a.collectionId) || null), { projectName: str(a.name), projectParent: str(a.parentDir) }));
      return;
    case "projects.cloneProject":
      // The repository URL can carry credentials, so it is handed over in memory instead of the address bar.
      presetCloneUrl(str(a.url));
      go(withParams(projectDialogUrl("clone"), { projectName: str(a.folderName), projectParent: str(a.parentDir), projectBranch: str(a.branch) }));
      return;
    case "projects.newSession": {
      const target = { projectId: str(a.projectId), groupId: str(a.groupId) || null };
      if (a.sessionKind === "terminal") { await store.newScratchTab({ target }); return; }
      go(agentPickerUrl({ ...target, requestId: genId(), anchorId: null, placement: "sibling", search: "", choice: `kind:${str(a.sessionKind)}` }));
      return;
    }
    case "projects.openSession": store.openSession(str(a.sessionId)); return;
    case "projects.revealProject": showProjects(); await store.revealFreshProject(str(a.projectId)); return;

    case "files.openFile": store.openDocTab(str(a.path)); return;
    case "files.reveal": await revealPath(str(a.path)); return;

    case "knowledge.openVault": go(memoryUrl(notebookRoute(str(a.vaultId)))); return;
    case "knowledge.search": go(memoryUrl(notebookRoute(str(a.vaultId)), { memoryQuery: str(a.query), memoryScope: a.allVaults ? "all" : null })); return;
    case "knowledge.openNote": openVaultFile(str(a.absolutePath)); return;
    case "knowledge.createNote": go(memoryUrl(notebookRoute(str(a.vaultId), "new"), { memoryName: str(a.name) || null, memoryFolder: str(a.folder) || null })); return;
    case "knowledge.createFolder": go(memoryUrl(notebookRoute(str(a.vaultId), "folder"), { memoryName: str(a.name) || null, memoryFolder: str(a.folder) || null })); return;
    case "knowledge.moveNote": go(memoryUrl(notebookRoute(str(a.vaultId), "move"), { memoryPath: str(a.path) })); return;
    case "knowledge.deleteNote": go(memoryUrl(notebookRoute(str(a.vaultId), "delete"), { memoryPath: str(a.path) })); return;
    case "knowledge.importFiles": go(memoryUrl(notebookRoute(str(a.vaultId), "import"), { memoryFolder: str(a.folder) || null })); return;
    case "knowledge.createVault": go(memoryUrl("notebooks", { kbAction: "new" })); return;
    case "knowledge.openCollections": go(memoryUrl("collections")); return;

    case "settings.openSettings": go(translationUrl({ settings: str(a.page), settingsSection: null, settingsTranslation: a.page === "translation" ? "1" : null, translationService: null })); return;
    case "settings.set": {
      const value = a.value;
      if (a.setting === "theme") store.setTheme(value as Theme);
      else if (a.setting === "language") setLang(value as LangChoice);
      else if (a.setting === "accent") store.setAccent(value as AccentChoice);
      else if (a.setting === "density") store.setDensity(value as Density);
      else if (a.setting === "sound") { if (store.soundEnabled !== value) store.toggleSound(); }
      else if (a.setting === "notifications") { if (store.notifyEnabled !== value) store.toggleNotify(); }
      return;
    }
    case "settings.setFontSize": {
      const size = Number(a.size);
      if (a.target === "terminal") store.setTermFontSize(size); else if (a.target === "conversation") store.setChatFontSize(size); else store.setUiFontSize(size);
      return;
    }
    case "settings.setFontFamily": {
      const family = str(a.family) || null;
      if (a.target === "conversation") store.setChatFontFamily(family);
      else if (a.target === "interface") store.setUiFontFamily(family);
      else store.setTermFontFamily(family);
      return;
    }
  }
  throw new Error("unsupported");
}

/** Fields that name an item. A value starting with `$` names an item an earlier action has yet to create. */
const ID_FIELDS = ["id", "projectId", "groupId", "parentGroupId", "parentSessionId", "collectionId", "parentId", "sessionId"];

/** The `$name`s of items an earlier action of the request has yet to create; the host fills in real IDs. */
export function references(action: AssistAction): string[] {
  const a = action as Record<string, unknown>;
  const ids = [...ID_FIELDS.map(key => a[key]), ...(Array.isArray(a.ids) ? a.ids : [])];
  return ids.filter((value): value is string => typeof value === "string" && value.startsWith("$"));
}

/** True while the action needs an item that an earlier, still unconfirmed action of the request creates. */
export const waitsForEarlier = (action: AssistAction) => references(action).length > 0;

/** Refreshes views that do not watch the host for this kind of change. */
export function afterHostAction(action: AssistAction) {
  const a = action as Record<string, unknown> & AssistAction;
  if (a.domain === "knowledge") window.dispatchEvent(new Event("notebook:vaultsChanged"));
  if (a.domain === "projects" && a.kind === "importProject") {
    const project = a.result as { id?: string } | undefined;
    if (project?.id) { showProjects(); void useTermStore.getState().revealFreshProject(project.id); }
  }
}
