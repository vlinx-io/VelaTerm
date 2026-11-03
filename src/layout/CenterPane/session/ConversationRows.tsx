//! The rows of a conversation as the session view draws them, shared by the conversation pane and the
//! task tab that shows a subagent's own conversation.

import { memo, type ReactNode } from "react";

import { useT } from "../../../i18n";
import type { ChatImageValue, ChatRewindScope, ChatRow } from "../../../ipc/chat";
import { mapBackendError } from "../../../ipc/backendError";
import { kindIconEl } from "../../sessionViewers/sessionMeta";
import { messageSenderName } from "./MessageSender";
import {
  CompactionRow,
  CommandRow,
  ErrorRow,
  MessageBubble,
  NoticeRow,
  ReasoningRow,
  ShellRow,
  ToolCard,
  ToolRunCard,
  TurnHead,
  WakeRow,
} from "./rows";
import type { DisplayRow, TurnFold } from "./toolRuns";
import { DeferredBody } from "./DeferredContents";

/** A user message edited before it is sent again from a rewind. */
export interface MessageReplacement {
  text: string;
  images: ChatImageValue[];
}

/** One entry of the list: a single row, or a folded run of tool calls. */
export const Entry = memo(function Entry({
  entry,
  label,
  icon,
  cwd,
  openRuns,
  onToggleRun,
  onToggleFold,
  onRewind,
  rewindDisabledReason,
  rewindScopes,
  rewindRequest,
  onRewindRequestHandled,
  onCancelShell,
  feedback,
}: {
  entry: DisplayRow;
  label: string;
  icon: ReactNode;
  cwd?: string;
  openRuns: ReadonlySet<string>;
  onToggleRun: (id: string) => void;
  onToggleFold?: (fold: TurnFold) => void;
  onRewind?: (rowId: string, scope: ChatRewindScope, replacement?: MessageReplacement) => void;
  rewindDisabledReason?: string;
  rewindScopes: ChatRewindScope[];
  rewindRequest: { rowId: string; token: number } | null;
  onRewindRequestHandled: (token: number) => void;
  onCancelShell?: (rowId: string) => void;
  feedback?: ReactNode;
}) {
  // The author line the pass in `toolRuns` placed on the first entry of an agent turn. Everything the
  // agent did in that turn sits under it, so reasoning and tool calls read as the agent's.
  const head = entry.head ? (
    <TurnHead
      who={entry.head.who ?? label}
      icon={icon}
      at={entry.head.at}
      durationMs={entry.head.durationMs}
      fold={entry.head.fold}
      onToggleFold={onToggleFold}
    />
  ) : null;
  if (entry.kind === "fold") return head;
  if (entry.kind === "run") {
    return (
      <>
        {head}
        <ToolRunCard
          calls={entry.calls}
          renderCall={row => <Row row={row} label={label} icon={icon} cwd={cwd} />}
          running={entry.running}
          open={openRuns.has(entry.id)}
          onToggle={() => onToggleRun(entry.id)}
          cwd={cwd}
        />
      </>
    );
  }
  return (
    <>
      {head}
      <Row
        row={entry.row}
        label={label}
        icon={icon}
        cwd={cwd}
        headless
        onRewind={onRewind}
        rewindDisabledReason={rewindDisabledReason}
        rewindScopes={rewindScopes}
        rewindRequest={rewindRequest}
        onRewindRequestHandled={onRewindRequestHandled}
        onCancelShell={onCancelShell}
        feedback={feedback}
      />
    </>
  );
});

/** One row of the live conversation. */
export function Row({
  row,
  label,
  icon,
  cwd,
  headless = false,
  onRewind,
  rewindDisabledReason,
  rewindScopes,
  rewindRequest,
  onRewindRequestHandled,
  onCancelShell,
  feedback,
}: {
  row: ChatRow;
  label: string;
  icon: ReactNode;
  cwd?: string;
  /** True when the turn's author line already stands above this row; subagent rows keep their own. */
  headless?: boolean;
  onRewind?: (rowId: string, scope: ChatRewindScope, replacement?: MessageReplacement) => void;
  rewindDisabledReason?: string;
  rewindScopes?: ChatRewindScope[];
  rewindRequest?: { rowId: string; token: number } | null;
  onRewindRequestHandled?: (token: number) => void;
  /** Stop a running shell command; the row knows only its own id. */
  onCancelShell?: (rowId: string) => void;
  feedback?: ReactNode;
}) {
  const t = useT();
  switch (row.kind) {
    case "tool": {
      // A subagent's own work, drawn with the same components as the conversation: it is the same kind
      // of thing, only reported under the call that started it.
      const steps = row.children?.length ? (
        <div className="sv-subagent">
          {row.children.map((child) => (
            <Row key={child.id} row={child} label={label} icon={icon} cwd={cwd} />
          ))}
        </div>
      ) : undefined;
      return (
        <ToolCard
          source={row}
          renderChildren={children => <div className="sv-subagent">{children.map(child => <Row key={child.id} row={child} label={label} icon={icon} cwd={cwd} />)}</div>}
          name={row.name}
          input={row.input}
          output={row.output}
          isError={row.isError}
          running={row.status === "running"}
          cwd={cwd}
          steps={steps}
          stepCount={row.childCount ?? row.children?.length}
          subagent={row.subagent}
        />
      );
    }
    case "reasoning":
      return <><ReasoningRow text={row.text} /><DeferredBody row={row} /></>;
    case "error":
      // Agents may record a stable backend code instead of prose, such as ZCode's unavailable plan.
      return <ErrorRow message={mapBackendError(row.message)} />;
    case "command":
      return <CommandRow text={row.text} />;
    case "shell":
      return <ShellRow row={row} onCancel={onCancelShell} />;
    case "notice":
      return <NoticeRow message={row.message} />;
    case "wake":
      return <WakeRow summary={row.summary} at={row.at} />;
    case "compaction":
      return (
        <CompactionRow
          done={row.status === "completed"}
          trigger={row.trigger}
          preTokens={row.preTokens}
        />
      );
    case "user":
      return (
        <MessageBubble
          who={row.origin ? messageSenderName(row.origin, t) : t("archive.you")}
          icon={row.origin ? kindIconEl(row.origin.agent, 14) : undefined}
          isUser
          rowId={row.id}
          text={row.text}
          copyAvailable={!row.bodyAvailable}
          images={row.images}
          at={row.at}
          onRewind={onRewind ? (scope) => onRewind(row.id, scope) : undefined}
          onEditSend={onRewind && !row.bodyAvailable ? (text, images) => onRewind(row.id, "conversation", { text, images }) : undefined}
          footnote={<><DeferredBody row={row} />{feedback}</>}
          rewindDisabledReason={rewindDisabledReason || (rewindScopes?.length ? undefined : t("chat.rewind.unsupported"))}
          editDisabledReason={rewindDisabledReason || (rewindScopes?.includes("conversation") ? undefined : t("chat.rewind.unsupported"))}
          rewindScopes={rewindScopes}
          openRewindToken={rewindRequest?.rowId === row.id ? rewindRequest.token : undefined}
          onRewindMenuOpened={onRewindRequestHandled}
        />
      );
    default:
      return (
        <MessageBubble
          who={row.model ?? label}
          isUser={false}
          streaming={row.streaming}
          rowId={row.id}
          icon={icon}
          text={row.text}
          at={row.at}
          durationMs={row.durationMs}
          showHead={!headless}
          footnote={<DeferredBody row={row} />}
        />
      );
  }
}
