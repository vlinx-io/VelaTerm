//! Small helpers for background tasks shared by the task tab, its tab-bar entry and the run strip. They live
//! apart from TaskView so those always-visible parts do not pull the task view and its conversation rows
//! into the entry bundle.

import Icons from "../../../components/Icons";

/** The icon a task kind is drawn with, in the task view and on its tab. */
export function taskIcon(taskType: string) {
  if (taskType === "local_workflow") return Icons.layers;
  if (taskType === "local_agent") return Icons.bot;
  return Icons.terminal;
}

/** "1:05" or "1:02:03": the duration format the phase list and the elapsed figure share. */
export function fmtElapsed(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const seconds = total % 60;
  const minutes = Math.floor(total / 60) % 60;
  const hours = Math.floor(total / 3600);
  const mm = hours ? String(minutes).padStart(2, "0") : String(minutes);
  return `${hours ? `${hours}:` : ""}${mm}:${String(seconds).padStart(2, "0")}`;
}
