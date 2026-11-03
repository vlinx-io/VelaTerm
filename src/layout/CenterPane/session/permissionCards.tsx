//! The cards shown when the agent is waiting for an answer.
//!
//! Most tools need nothing but a yes or a no, and get the plain card at the bottom of this file. Two do
//! not, and they are the two that otherwise stall a conversation:
//!
//! - **AskUserQuestion** is a question, so it needs a form. The picks travel back inside the tool's own
//!   arguments — see `questionForm.ts` — which is why answering is an "allow" with an edited input rather
//!   than a message.
//! - **ExitPlanMode** hands over a plan written in Markdown. Shown as a JSON blob it is unreadable, so it
//!   gets rendered, and approving it also drops the conversation out of plan mode.
//!
//! The plain card has a third kind of answer besides yes and no: the standing rules the agent offers with
//! its question — "accept file edits for the rest of this session", "always allow this command". Each is
//! a button that allows the tool and adopts the rule in one press, so the same question stops coming back.

import { useMemo, useRef, useState } from "react";

import Icons from "../../../components/Icons";
import Select from "../../../components/Select";
import Combo from "../../../components/Combo";
import { useT } from "../../../i18n";
import type { ChatPermission, QuestionAnswerSelection } from "../../../ipc/chat";
import { QuestionText, useQuestionTranslation } from "../../../translation/Question";
import { translationError } from "../../../translation/SettingsSection";
import { Markdown } from "./markdown";
import { parsePermissionSuggestions, type Mode, type SuggestionOffer } from "./permissions";
import {
  areQuestionsAnswered,
  buildAnsweredInput,
  isQuestionAnswered,
  parsePlanText,
  parseQuestionFormQuestions,
  questionShowsTextInput,
  shouldSubmitEmptyOnDismiss,
  type QuestionFormQuestion,
} from "./questionForm";
import { ToolCard } from "./rows";
import { useMessageCopyMenu } from "./useMessageCopyMenu";
import {
  buildElicitationContent,
  initialElicitationValues,
  isElicitationComplete,
  isElicitationVisible,
  parseElicitationFields,
  type ElicitationField,
  type ElicitationValue,
  type ElicitationValues,
} from "./elicitationForm";

/** How a card answers: allow or deny, optionally with edited arguments, a reason, or a mode to switch to. */
export interface PermissionAnswer {
  allow: boolean;
  /** Arguments to run the tool with, when the answer is part of them. */
  updatedInput?: unknown;
  /** Why it was refused, shown to the agent. */
  message?: string;
  /** Permission mode to switch the conversation to before answering. */
  mode?: Mode;
  /** Standing rules the answer adopts, taken from the ones the question offered. */
  updatedPermissions?: unknown[];
  questionAnswers?: QuestionAnswerSelection[];
  sendOriginal?: boolean;
}

export interface PermissionCardProps {
  request: ChatPermission;
  cwd?: string;
  onAnswer: (request: ChatPermission, answer: PermissionAnswer) => void | Promise<void>;
}

/** A tool waiting for permission, shown above the composer where an answer is expected. */
export function PermissionCard({ request, cwd, onAnswer }: PermissionCardProps) {
  const questions = useMemo(
    () =>
      request.tool_name === "AskUserQuestion"
        ? parseQuestionFormQuestions(request.input)
        : null,
    [request.tool_name, request.input],
  );
  const plan = useMemo(
    () => (request.tool_name === "ExitPlanMode" ? parsePlanText(request.input) : null),
    [request.tool_name, request.input],
  );

  // A malformed request falls through to the plain card rather than showing nothing: the agent is still
  // waiting, and a yes/no is better than a dead end.
  if (request.subtype === "elicitation") return <ElicitationCard request={request} onAnswer={onAnswer} />;
  if (questions) return <QuestionCard request={request} questions={questions} onAnswer={onAnswer} />;
  if (plan) return <PlanCard request={request} plan={plan} onAnswer={onAnswer} />;
  return <ToolPermissionCard request={request} cwd={cwd} onAnswer={onAnswer} />;
}

/**
 * An MCP server asking the person something, through the agent.
 *
 * Two shapes exist. A form carries a flat JSON schema, drawn field by field; a URL asks the person to
 * complete something in the browser and say so. Both take three answers: the form (or "done"), a
 * refusal, and a plain cancel that closes the request without either.
 */
function ElicitationCard({
  request,
  onAnswer,
}: {
  request: ChatPermission;
  onAnswer: PermissionCardProps["onAnswer"];
}) {
  const t = useT();
  const fields = useMemo(() => parseElicitationFields(request.requested_schema), [request.requested_schema]);
  const [values, setValues] = useState<ElicitationValues>(() => initialElicitationValues(fields));
  const isUrl = request.mode === "url" && typeof request.url === "string";
  const complete = isUrl || isElicitationComplete(fields, values);
  const server = request.mcp_server_name ?? "";
  const title = request.title ?? request.display_name;
  const set = (name: string, value: ElicitationValue) => setValues((prev) => ({ ...prev, [name]: value }));
  return (
    <div className="sv-permission">
      <div className="sv-permission-head">
        <Icons.connect size={13} />
        <span className="sv-permission-title">{title ? title : t("chat.elicitation.heading", server)}</span>
        {title ? <span className="sv-elicit-server">{server}</span> : null}
      </div>
      {request.message ? (
        <div className="sv-elicit-message">
          <Markdown text={request.message} />
        </div>
      ) : null}
      {isUrl ? (
        <a className="sv-elicit-url" href={request.url} target="_blank" rel="noreferrer">
          <Icons.external size={12} />
          {request.url}
        </a>
      ) : (
        fields.filter((field) => isElicitationVisible(field, fields, values)).map((field) => (
          <ElicitationInput key={field.name} field={field} value={values[field.name]} onChange={set} />
        ))
      )}
      <div className="sv-permission-actions">
        <button className="sv-deny" onClick={() => onAnswer(request, { allow: false, message: "cancel" })}>
          {t("chat.elicitation.cancel")}
        </button>
        <button className="sv-deny" onClick={() => onAnswer(request, { allow: false })}>
          {t("chat.elicitation.decline")}
        </button>
        <button
          className="sv-allow"
          disabled={!complete}
          onClick={() =>
            onAnswer(request, {
              allow: true,
              updatedInput: isUrl ? undefined : buildElicitationContent(fields, values),
            })
          }
        >
          {isUrl ? t("chat.elicitation.done") : t("chat.elicitation.submit")}
        </button>
      </div>
    </div>
  );
}

function ElicitationInput({
  field,
  value,
  onChange,
}: {
  field: ElicitationField;
  value: ElicitationValue | undefined;
  onChange: (name: string, value: ElicitationValue) => void;
}) {
  const t = useT();
  const label = (
    <span className="sv-elicit-label">
      {field.label}
      {field.required ? <span className="sv-elicit-required">*</span> : null}
    </span>
  );
  const hint = field.description ? <span className="sv-elicit-hint">{field.description}</span> : null;
  if (field.kind === "multiselect") {
    const selected = Array.isArray(value) ? value : [];
    return <div className="sv-elicit-field">
      {label}{hint}
      {(field.options ?? []).map((option) => <label key={option.value} className="sv-elicit-check">
        <input type="checkbox" checked={selected.includes(option.value)} onChange={(e) => onChange(field.name,
          e.target.checked ? [...selected, option.value] : selected.filter((v) => v !== option.value))} />
        {option.label}
      </label>)}
      {field.custom ? <textarea className="sv-question-other" aria-label={field.label} rows={2}
        value={selected.filter((v) => !field.options?.some((o) => o.value === v)).join("\n")}
        onChange={(e) => onChange(field.name, [...selected.filter((v) => field.options?.some((o) => o.value === v)),
          ...e.target.value.split("\n").filter((v) => v.trim() !== "")])} /> : null}
    </div>;
  }
  if (field.kind === "boolean") {
    return (
      <label className="sv-elicit-field sv-elicit-check">
        <input type="checkbox" checked={value === true} onChange={(e) => onChange(field.name, e.target.checked)} />
        {field.url ? <a href={field.url} target="_blank" rel="noreferrer">{field.label}</a> : label}
        {hint}
      </label>
    );
  }
  const text = typeof value === "string" ? value : "";
  if (field.kind === "enum") {
    return (
      <label className="sv-elicit-field">
        {label}
        {hint}
        {field.custom ? <Combo value={text} onChange={(value) => onChange(field.name, value)} allowCustom
          options={(field.options ?? []).map((option) => ({ value: option.value, label: option.label }))} />
          : <Select width="100%" value={text} ariaLabel={field.label} onChange={(value) => onChange(field.name, value)} options={[{ value: "", label: t("chat.elicitation.choose") }, ...(field.options?.map((option) => ({ value: option.value, label: option.label })) ?? [])]} />}
      </label>
    );
  }
  if (field.multiline) {
    return (
      <label className="sv-elicit-field">
        {label}
        {hint}
        <textarea
          className="sv-question-other"
          rows={3}
          value={text}
          onChange={(e) => onChange(field.name, e.target.value)}
        />
      </label>
    );
  }
  return (
    <label className="sv-elicit-field">
      {label}
      {hint}
      <input
        className="sv-question-other"
        type={field.kind === "text" ? "text" : "number"}
        step={field.kind === "integer" ? 1 : "any"}
        value={text}
        onChange={(e) => onChange(field.name, e.target.value)}
      />
    </label>
  );
}

/** Chosen option indices, by question index. */
type Selections = Record<number, Set<number>>;

/** Whether the current text selection reaches into `element`, including one that lies wholly inside it. */
function selectsTextIn(element: Element): boolean {
  const selection = window.getSelection();
  return !!selection && !selection.isCollapsed && selection.rangeCount > 0 && selection.getRangeAt(0).intersectsNode(element);
}

/**
 * A question the agent asked, as a form.
 *
 * One question at a time, with a tab per question when there are several: the questions are usually short
 * and related, and a wall of them above the composer buries the last one. The primary button walks forward
 * through them and only submits on the last, so nothing is sent half-answered.
 */
function QuestionCard({
  request,
  questions,
  onAnswer,
}: {
  request: ChatPermission;
  questions: QuestionFormQuestion[];
  onAnswer: PermissionCardProps["onAnswer"];
}) {
  const t = useT();
  const translation = useQuestionTranslation(request);
  const { onContextMenu, copyMenu } = useMessageCopyMenu();
  const [selections, setSelections] = useState<Selections>({});
  const [otherTexts, setOtherTexts] = useState<Record<number, string>>({});
  const [active, setActive] = useState(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const submitting = useRef(false);

  const index = Math.min(active, questions.length - 1);
  const question = questions[index];
  const selected = selections[index] ?? new Set<number>();
  const otherText = otherTexts[index] ?? "";
  const isLast = index === questions.length - 1;
  const answered = isQuestionAnswered(question, index, selections, otherTexts);
  const allAnswered = areQuestionsAnswered(questions, selections, otherTexts);

  // Picking clears anything typed for the same question, and typing clears the picks: an answer is one or
  // the other, and leaving both would make the submitted value depend on a rule nobody can see.
  const toggle = (option: number) => {
    setSelections((prev) => {
      const next = new Set(prev[index] ?? []);
      if (question.multiSelect) {
        if (next.has(option)) next.delete(option);
        else next.add(option);
      } else if (next.has(option)) {
        next.clear();
      } else {
        next.clear();
        next.add(option);
      }
      return { ...prev, [index]: next };
    });
    setOtherTexts((prev) => (prev[index] ? { ...prev, [index]: "" } : prev));
    // A single-choice answer is final the moment it is picked, so move on rather than making someone
    // press Next for a decision they have already made.
    if (!question.multiSelect) setActive(Math.min(index + 1, questions.length - 1));
  };

  const type = (text: string) => {
    setOtherTexts((prev) => ({ ...prev, [index]: text }));
    if (text.length > 0) {
      setSelections((prev) => (prev[index]?.size ? { ...prev, [index]: new Set<number>() } : prev));
    }
  };

  const submit = async (sendOriginal = false) => {
    if (!allAnswered || submitting.current) return;
    submitting.current = true; setBusy(true); setError("");
    try {
      await onAnswer(request, {
        allow: true,
        questionAnswers: questions.map((_, i) => ({ selected: [...(selections[i] ?? [])], text: otherTexts[i] ?? "" })),
        sendOriginal,
      });
    } catch (error) { setError(translationError(error, t)); }
    finally { submitting.current = false; setBusy(false); }
  };

  const dismiss = () => {
    if (submitting.current) return;
    // Closing a question that asked for nothing in particular is an empty answer, not a refusal: the
    // agent should carry on rather than record a tool it was not allowed to run.
    if (shouldSubmitEmptyOnDismiss(questions)) {
      onAnswer(request, {
        allow: true,
        updatedInput: buildAnsweredInput(request.input, questions, selections, otherTexts),
      });
      return;
    }
    onAnswer(request, { allow: false, message: "Dismissed by user" });
  };

  const primary = () => {
    if (submitting.current) return;
    if (isLast) void submit();
    else if (answered) setActive(index + 1);
  };

  return (
    <div className="sv-permission sv-question" onContextMenu={onContextMenu}>
      <div className="sv-permission-head">
        <Icons.info size={13} />
        <span className="sv-permission-title">{t("chat.question.heading")}</span>
      </div>

      {translation.control}

      {questions.length > 1 && (
        <div className="sv-question-tabs" role="tablist">
          {questions.map((q, i) => (
            <button
              key={q.header}
              role="tab"
              aria-selected={i === index}
              className={"sv-question-tab" + (i === index ? " sv-question-tab-on" : "")}
              onClick={() => setActive(i)}
            >
              {isQuestionAnswered(q, i, selections, otherTexts) ? <Icons.check size={11} /> : null}
              <QuestionText original={q.header} translated={translation.questions?.[i]?.header} mode={translation.mode} target={translation.target} />
            </button>
          ))}
        </div>
      )}

      <div className="sv-question-text"><QuestionText original={question.question} translated={translation.questions?.[index]?.question} mode={translation.mode} target={translation.target} /></div>

      {question.options.length > 0 && (
        <div className="sv-question-options" role={question.multiSelect ? undefined : "radiogroup"}>
          {question.options.map((option, i) => (
            // Not a <button>: text inside one cannot be selected reliably across engines, and the wording
            // of an option is often worth copying.
            <div
              key={option.label}
              role={question.multiSelect ? "checkbox" : "radio"}
              aria-checked={selected.has(i)}
              aria-disabled={busy || undefined}
              tabIndex={busy ? -1 : 0}
              className={"sv-question-option" + (selected.has(i) ? " sv-question-option-on" : "")}
              onClick={(e) => {
                // A drag that selected part of the option was a copy, not a pick.
                if (!busy && !selectsTextIn(e.currentTarget)) toggle(i);
              }}
              onKeyDown={(e) => {
                if (busy || (e.key !== "Enter" && e.key !== " ")) return;
                e.preventDefault();
                toggle(i);
              }}
            >
              <span
                className={
                  "sv-question-mark" +
                  (question.multiSelect ? " sv-question-mark-box" : " sv-question-mark-dot")
                }
              >
                {selected.has(i) && question.multiSelect ? <Icons.check size={10} /> : null}
              </span>
              <span className="sv-question-option-text">
                <span className="sv-question-option-label"><QuestionText original={option.label} translated={translation.questions?.[index]?.options[i]?.label} mode={translation.mode} target={translation.target} /></span>
                {option.description ? (
                  <span className="sv-question-option-desc"><QuestionText original={option.description} translated={translation.questions?.[index]?.options[i]?.description} mode={translation.mode} target={translation.target} /></span>
                ) : null}
              </span>
            </div>
          ))}
        </div>
      )}

      {questionShowsTextInput(question) && (
        <input
          className="sv-question-other"
          value={otherText}
          disabled={busy}
          aria-label={question.question}
          placeholder={
            (translation.mode !== "original" ? translation.questions?.[index]?.placeholder : undefined) ?? question.placeholder ??
            (question.options.length === 0
              ? t("chat.question.answerPlaceholder")
              : t("chat.question.otherPlaceholder"))
          }
          onChange={(e) => type(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.nativeEvent.isComposing) {
              e.preventDefault();
              primary();
            }
          }}
        />
      )}

      {busy && <div className="translation-message-status" role="status">{t(translation.outgoing && Object.values(otherTexts).some(text => text.trim()) ? "translation.translating" : "common.loading")}</div>}
      {error && <div className="translation-message-status" role="alert">{error}
        <button className="vlx-btn" onClick={() => void submit()}>{t("common.retry")}</button>
        {translation.outgoing && <button className="vlx-btn" onClick={() => void submit(true)}>{t("translation.sendOriginal")}</button>}
      </div>}

      <div className="sv-permission-actions">
        <button className="sv-deny" disabled={busy} onClick={dismiss}>
          {t("chat.question.dismiss")}
        </button>
        <button
          className="sv-allow"
          disabled={busy || (isLast ? !allAnswered : !answered)}
          onClick={primary}
        >
          {isLast ? t("chat.question.submit") : t("chat.question.next")}
        </button>
      </div>
      {copyMenu}
    </div>
  );
}

/**
 * A plan the agent wants to carry out.
 *
 * Approving it is also the moment plan mode ends, so the card switches the conversation to accepting file
 * edits. Anything else would leave someone confirming every write of a plan they just approved wholesale.
 */
function PlanCard({
  request,
  plan,
  onAnswer,
}: {
  request: ChatPermission;
  plan: string;
  onAnswer: (request: ChatPermission, answer: PermissionAnswer) => void;
}) {
  const t = useT();
  return (
    <div className="sv-permission sv-plan">
      <div className="sv-permission-head">
        <Icons.docLines size={13} />
        <span className="sv-permission-title">{t("chat.plan.heading")}</span>
      </div>
      <div className="sv-plan-body">
        <Markdown text={plan} />
      </div>
      <div className="sv-permission-actions">
        <button
          className="sv-deny"
          onClick={() => onAnswer(request, { allow: false, message: "Plan rejected by user" })}
        >
          {t("chat.plan.reject")}
        </button>
        <button
          className="sv-allow"
          onClick={() => onAnswer(request, { allow: true, mode: "acceptEdits" })}
        >
          {t("chat.plan.implement")}
        </button>
      </div>
    </div>
  );
}

/** Any other tool: what it is about to do, a yes or a no, and whatever standing rules were offered. */
function ToolPermissionCard({ request, cwd, onAnswer }: PermissionCardProps) {
  const t = useT();
  const name = request.display_name ?? request.tool_name;
  const offers = useMemo(
    () => parsePermissionSuggestions(request.permission_suggestions),
    [request.permission_suggestions],
  );
  // Shown as the agent wrote it: it names the check that stopped the call, which is what explains a
  // prompt that arrives even though the chosen mode would normally allow the tool.
  const reason = typeof request.decision_reason === "string" ? request.decision_reason.trim() : "";
  return (
    <div className="sv-permission">
      <div className="sv-permission-head">
        <Icons.lock size={13} />
        <span className="sv-permission-title">{t("chat.permissionAsk", name ?? "")}</span>
      </div>
      {reason && <div className="sv-permission-reason">{reason}</div>}
      <ToolCard
        name={request.tool_name}
        input={request.input}
        isError={false}
        running={false}
        cwd={cwd}
      />
      {offers.length > 0 && (
        <div className="sv-suggestions">
          {offers.map((offer, i) => (
            <SuggestionButton
              key={i}
              offer={offer}
              onTake={() =>
                onAnswer(request, {
                  allow: true,
                  updatedPermissions: [offer.suggestion],
                  // A mode switch also travels the way the mode chip travels, so the chip, the stored
                  // session setting and the agent all end up saying the same thing.
                  mode: offer.mode,
                })
              }
            />
          ))}
        </div>
      )}
      <div className="sv-permission-actions">
        <button className="sv-deny" onClick={() => onAnswer(request, { allow: false })}>
          {t("chat.deny")}
        </button>
        <button className="sv-allow" onClick={() => onAnswer(request, { allow: true })}>
          {t("chat.allow")}
        </button>
      </div>
    </div>
  );
}

/**
 * One standing rule, as a button that allows the tool and adopts the rule together.
 *
 * The wording says how long it lasts, because that is the part someone has to weigh: a session rule is
 * forgotten when the conversation ends, while the others are written into a settings file and outlive it.
 */
function SuggestionButton({ offer, onTake }: { offer: SuggestionOffer; onTake: () => void }) {
  const t = useT();
  let label: string;
  if (offer.kind === "mode") {
    // Every mode reaching here has a name of its own; `permissions.ts` drops the ones that do not.
    const mode = t(`chat.mode.${offer.mode}` as "chat.mode.default");
    label = offer.session ? t("chat.suggest.modeSession", mode) : t("chat.suggest.mode", mode);
  } else if (offer.kind === "rules") {
    const rule = offer.subject ?? "";
    label = offer.session ? t("chat.suggest.allowSession", rule) : t("chat.suggest.allowAlways", rule);
  } else if (offer.kind === "network") {
    label = t("chat.suggest.networkAlways", offer.subject ?? "");
  } else {
    const dirs = offer.subject ?? "";
    label = offer.session ? t("chat.suggest.dirSession", dirs) : t("chat.suggest.dirAlways", dirs);
  }
  return (
    <button className="sv-suggest" title={label} onClick={onTake}>
      {label}
    </button>
  );
}
