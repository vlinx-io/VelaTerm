//! Name of a changes tab, shared by the tab bar and the tab's own header.

import type { t as translate } from "../../i18n";
import type { ChangesTarget } from "../../store/docTab";

export function changesTitle(t: typeof translate, target: ChangesTarget): string {
  const { side, commit, base } = target;
  return base && commit ? t("git.branchChanges") : commit ? t("changes.commitTitle", commit.slice(0, 12))
    : side === "staged" ? t("git.staged") : side === "unstaged" ? t("git.changes")
    : side === "untracked" ? t("git.untracked") : side === "conflicted" ? t("git.conflicts") : t("changes.title");
}
