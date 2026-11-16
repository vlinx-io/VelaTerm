//! Clone Git Repository dialog: repository URL, Location, folder name, and optional branch. The clone lands in
//! `location/folder` and is imported as a project. Location is an editable path with Browse… (LocationField),
//! the same on every platform; the destination is checked live before Clone is enabled.

import "./folder-picker.css";
import { useEffect, useRef, useState } from "react";
import { genId } from "../genId";
import { useT } from "../i18n";
import { onCloneProgress, type CloneProgress } from "../ipc/events";
import { cancelCloneProject } from "../ipc/tree";
import { useTermStore } from "../store/termStore";
import { Backdrop } from "../components/Backdrop";
import Icons from "../components/Icons";
import { DestinationBox, LocationField, rememberProjectLocation, useDestination, useProjectLocation } from "./LocationField";
import { pushRecentFolder } from "./ServerFileBrowser";
import { ExecutionContext } from "./ExecutionContext";
import { trapDialogFocus, useDialogFocus } from "./dialogFocus";
import { takeCloneUrl, useDialogDraft, useDialogNavigationLock } from "./dialogNavigation";

export function CloneProjectModal() {
  const t = useT();
  const open = useTermStore((s) => s.cloneModalOpen);
  const setOpen = useTermStore((s) => s.setCloneModalOpen);
  const cloneProjectInto = useTermStore((s) => s.cloneProjectInto);

  const [url, setUrl] = useState("");
  const [branch, setBranch] = useDialogDraft("projectBranch", open);
  const [folder, setFolder] = useDialogDraft("projectName", open);
  const [folderTouched, setFolderTouched] = useState(false);
  const [location, setLocation] = useProjectLocation(open);
  const [cloning, setCloning] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [progress, setProgress] = useState<CloneProgress | null>(null);
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const [error, setError] = useState("");
  const operationId = useRef<string | null>(null);
  const cancelRequested = useRef(false);
  const startedAt = useRef(0);
  const lastProgressAt = useRef(0);
  const [branchOpen, setBranchOpen] = useState(false);
  const dialogRef = useDialogFocus(open);
  useDialogNavigationLock("clone", cloning);

  const dest = useDestination(location, folderTouched || folder ? folder : null, open && !cloning, url);
  const effectiveFolder = folderTouched || folder ? folder : dest.name;

  // Reset input state each time the dialog opens.
  useEffect(() => {
    if (open) {
      setUrl(takeCloneUrl());
      setFolderTouched(false);
      setBranchOpen(!!new URLSearchParams(window.location.search).get("projectBranch"));
      setCloning(false);
      setCancelling(false);
      setProgress(null);
      setElapsedSeconds(0);
      setError("");
      operationId.current = null;
      cancelRequested.current = false;
    }
  }, [open]);

  // The single global event carries an operationId; during concurrent clones, accept only the operation started by this dialog.
  useEffect(() => {
    const unlisten = onCloneProgress((next) => {
      if (next.operationId !== operationId.current) return;
      lastProgressAt.current = Date.now();
      setProgress(next);
    });
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  useEffect(() => {
    if (!cloning) return;
    const tick = () => {
      setElapsedSeconds(Math.max(0, Math.floor((Date.now() - startedAt.current) / 1000)));
    };
    tick();
    const timer = window.setInterval(tick, 1000);
    return () => window.clearInterval(timer);
  }, [cloning]);

  if (!open) return null;

  const canClone = !cloning && url.trim().length > 0 && dest.kind === "ok";

  const confirm = async () => {
    if (!canClone || dest.kind !== "ok") return;
    const parent = dest.parent;
    const currentOperationId = genId();
    operationId.current = currentOperationId;
    cancelRequested.current = false;
    startedAt.current = Date.now();
    lastProgressAt.current = startedAt.current;
    setCloning(true);
    setCancelling(false);
    setProgress(null);
    setElapsedSeconds(0);
    setError("");
    try {
      await cloneProjectInto(url.trim(), parent, effectiveFolder.trim(), branch.trim(), currentOperationId);
      rememberProjectLocation(parent);
      pushRecentFolder(parent);
    } catch (e) {
      const message = String(e);
      const wasCancelled = message === "CLONE_CANCELLED" || message === "Error: CLONE_CANCELLED";
      if (cancelRequested.current && wasCancelled) {
        setOpen(false);
      } else {
        setError(message);
      }
      setCloning(false);
      setCancelling(false);
    } finally {
      if (operationId.current === currentOperationId) operationId.current = null;
    }
  };

  const cancel = async () => {
    if (!cloning) {
      setOpen(false);
      return;
    }
    if (cancelling || !operationId.current) return;
    cancelRequested.current = true;
    setCancelling(true);
    setError("");
    try {
      await cancelCloneProject(operationId.current);
    } catch (e) {
      cancelRequested.current = false;
      setCancelling(false);
      setError(String(e));
    }
  };

  const stageText = (): string => {
    const label = (() => {
      switch (progress?.stage) {
        case "connecting":
          return t("clone.stageConnecting");
        case "preparing":
          return t("clone.stagePreparing");
        case "receiving":
          return t("clone.stageReceiving");
        case "resolving":
          return t("clone.stageResolving");
        case "checkout":
          return t("clone.stageCheckout");
        case "finalizing":
          return t("clone.stageFinalizing");
        case "importing":
          return t("clone.stageImporting");
        default:
          return t("clone.stageStarting");
      }
    })();
    return progress?.percent != null ? `${label} ${progress.percent}%` : label;
  };

  const progressStalled =
    cloning && elapsedSeconds >= 30 && Date.now() - lastProgressAt.current >= 30_000;

  return (
    <Backdrop onClose={() => !cloning && setOpen(false)} zIndex={300}>
      <form
        role="dialog"
        ref={dialogRef}
        aria-modal="true"
        aria-label={t("clone.title")}
        className="fp-dialog fp-form"
        tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          trapDialogFocus(e);
          if (e.key === "Escape" && !cloning) setOpen(false);
        }}
        onSubmit={(e) => {
          e.preventDefault();
          void confirm();
        }}
      >
        <div className="fp-head">
          <span className="fp-title">{t("clone.title")}</span>
          {!cloning && (
            <button type="button" className="icon-btn" aria-label={t("common.cancel")} title={t("common.cancel")} onClick={() => setOpen(false)}>
              <Icons.x size={14} />
            </button>
          )}
        </div>
        <ExecutionContext />

        <div className="fp-fields">
          <label className="fp-field">
            <span className="fp-label">{t("clone.url")}</span>
            <input
              className="vlx-input"
              autoFocus
              disabled={cloning}
              placeholder={t("clone.urlPlaceholder")}
              spellCheck={false}
              value={url}
              onChange={(e) => setUrl(e.target.value)}
            />
          </label>

          <div className="fp-field">
            <label className="fp-label" htmlFor="clone-location">{t("location.label")}</label>
            <LocationField
              id="clone-location"
              value={location}
              onChange={setLocation}
              disabled={cloning}
              invalid={dest.kind === "bad" && dest.field === "location"}
            />
          </div>

          <div className="fp-field">
            <label className="fp-field">
              <span className="fp-label">{t("clone.folder")}</span>
              <input
                className={"vlx-input" + (dest.kind === "bad" && dest.field === "name" ? " bad" : "")}
                disabled={cloning}
                placeholder={t("clone.folderPlaceholder")}
                spellCheck={false}
                value={effectiveFolder}
                onChange={(e) => {
                  setFolderTouched(true);
                  setFolder(e.target.value);
                }}
              />
            </label>
          </div>

          <div className="fp-branch">
            <button type="button" className="fp-branch-toggle" aria-expanded={branchOpen} aria-controls="clone-branch"
              onClick={() => setBranchOpen(v => !v)}>
              {branchOpen ? <Icons.chevD size={16} /> : <Icons.chevR size={16} />}
              <span>{t("clone.branch")}<small>{branch || t("clone.defaultBranch")}</small></span>
            </button>
            {branchOpen && <label className="fp-field" id="clone-branch">
              <span className="fp-label">{t("clone.branch")}</span>
              <input
                className="vlx-input"
                disabled={cloning}
                placeholder={t("clone.branchPlaceholder")}
                spellCheck={false}
                value={branch}
                onChange={(e) => setBranch(e.target.value)}
              />
            </label>}
          </div>

          <DestinationBox dest={dest} label={t("clone.destination")} readyLabel={t("clone.ready")} />
          {cloning && (
            <div className="fp-progress" aria-live="polite">
              <div className="fp-progress-top">
                <span className="fp-spin" />
                <span>{cancelling ? t("clone.cancelling") : stageText()}</span>
                <span className="fp-elapsed">{t("clone.elapsed", elapsedSeconds)}</span>
              </div>
              <div className="fp-bar-track">
                <div
                  className={"fp-bar-fill" + (progress?.percent == null ? " indeterminate" : "")}
                  style={progress?.percent != null ? { width: `${progress.percent}%` } : undefined}
                />
              </div>
              {progressStalled && <div className="fp-progress-hint">{t("clone.slowHint")}</div>}
            </div>
          )}

          {error && <div className="fp-error">{error}</div>}
        </div>

        <div className="fp-foot">
          <span className="fp-spacer" />
          <button type="button" className="vlx-btn" onClick={() => void cancel()} disabled={cancelling}>
            {cloning ? (cancelling ? t("clone.cancelling") : t("clone.cancelClone")) : t("common.cancel")}
          </button>
          <button type="submit" className="vlx-btn vlx-btn-primary" disabled={!canClone}>
            {cloning ? t("clone.cloning") : t("clone.submit")}
          </button>
        </div>
      </form>
    </Backdrop>
  );
}
