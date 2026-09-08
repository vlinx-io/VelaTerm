//! Reading an AskUserQuestion request, writing the answer back into it, and reading it out again.
//!
//! The agent asks a question by proposing a tool call whose arguments *are* the question, and the host
//! answers by editing those arguments before letting the call run. So every direction lives here: parsing
//! the questions out of the proposed input, folding the picked answers back into a copy of it, and — once
//! the call has run — reading the question and its answer back out of the transcript for the card that
//! records the exchange.
//!
//! Ported from paseo's `question-form-card-core.ts`, with one deliberate difference. Paseo keys its answers
//! by each question's short header and remaps them to the full question text on the server; there is no
//! server in between here, so the answers are keyed by the question text from the start. The text is what
//! the tool expects — the same key its `annotations` use.

/** One choice offered for a question. */
export interface QuestionOption {
  label: string;
  description?: string;
}

/** One question, reduced to what the form needs to draw and validate it. */
export interface QuestionFormQuestion {
  question: string;
  /** Short label for the question, used as its tab. */
  header: string;
  options: QuestionOption[];
  multiSelect: boolean;
  /** Whether a free-text answer is offered alongside the options. */
  allowOther: boolean;
  /** Whether leaving the question blank counts as answering it. */
  allowEmpty: boolean;
  placeholder?: string;
}

/** Chosen option indices, by question index. */
export type QuestionSelections = Record<number, ReadonlySet<number>>;
/** Free-text answers, by question index. */
export type QuestionOtherTexts = Record<number, string>;

function readOptionalString(record: Record<string, unknown>, key: string): string | undefined {
  const value = record[key];
  return typeof value === "string" ? value : undefined;
}

/**
 * Read the questions out of an AskUserQuestion proposal, or null when it is not one this form can draw.
 *
 * "Other" is not something the agent offers: its own interface adds a free-text answer to every question,
 * so this does too unless the request explicitly rules it out.
 */
export function parseQuestionFormQuestions(input: unknown): QuestionFormQuestion[] | null {
  if (typeof input !== "object" || input === null) return null;
  const raw = (input as Record<string, unknown>).questions;
  if (!Array.isArray(raw)) return null;
  const questions: QuestionFormQuestion[] = [];
  for (const item of raw) {
    if (typeof item !== "object" || item === null) return null;
    const q = item as Record<string, unknown>;
    if (typeof q.question !== "string" || typeof q.header !== "string") return null;
    if (!Array.isArray(q.options)) return null;
    const options: QuestionOption[] = [];
    for (const opt of q.options as unknown[]) {
      if (typeof opt !== "object" || opt === null) return null;
      const o = opt as Record<string, unknown>;
      if (typeof o.label !== "string") return null;
      options.push({
        label: o.label,
        description: typeof o.description === "string" ? o.description : undefined,
      });
    }
    questions.push({
      question: q.question,
      header: q.header,
      options,
      multiSelect: q.multiSelect === true,
      allowOther: q.allowOther !== false,
      allowEmpty: q.allowEmpty === true,
      placeholder: readOptionalString(q, "placeholder"),
    });
  }
  return questions.length > 0 ? questions : null;
}

/** Whether this question takes a typed answer: it has no options at all, or "Other" is on offer. */
export function questionShowsTextInput(question: QuestionFormQuestion): boolean {
  return question.options.length === 0 || question.allowOther;
}

/** Whether one question has been answered, by a pick or by typing. */
export function isQuestionAnswered(
  question: QuestionFormQuestion,
  qIndex: number,
  selections: QuestionSelections,
  otherTexts: QuestionOtherTexts,
): boolean {
  const selected = selections[qIndex];
  if (selected && selected.size > 0) return true;
  if (!questionShowsTextInput(question)) return false;
  const typed = otherTexts[qIndex]?.trim();
  if (typed) return true;
  return question.allowEmpty;
}

/** Whether every question has been answered, which is what the submit button waits for. */
export function areQuestionsAnswered(
  questions: QuestionFormQuestion[] | null,
  selections: QuestionSelections,
  otherTexts: QuestionOtherTexts,
): boolean {
  return (
    questions?.every((question, i) => isQuestionAnswered(question, i, selections, otherTexts)) ??
    false
  );
}

/**
 * The answers, keyed by question text.
 *
 * A typed answer wins over the options, because typing one clears the picks: someone who writes their own
 * answer has left the menu behind. Several picks in a multi-select join into one line, which is the shape
 * the tool reads them in.
 */
export function buildQuestionFormAnswers(
  questions: QuestionFormQuestion[],
  selections: QuestionSelections,
  otherTexts: QuestionOtherTexts,
): Record<string, string> {
  const answers: Record<string, string> = {};
  for (let i = 0; i < questions.length; i++) {
    const q = questions[i];
    const selected = selections[i];
    const typed = otherTexts[i]?.trim();

    if (questionShowsTextInput(q)) {
      if (typed) {
        answers[q.question] = typed;
        continue;
      }
      if (q.allowEmpty && q.options.length === 0) {
        answers[q.question] = "";
        continue;
      }
    }

    if (selected && selected.size > 0) {
      answers[q.question] = Array.from(selected)
        .map((idx) => q.options[idx]?.label)
        .filter((label): label is string => typeof label === "string")
        .join(", ");
    }
  }
  return answers;
}

/**
 * The tool arguments to let the call run with: the proposal, plus the answers.
 *
 * Everything the agent proposed is kept — the tool validates its own full schema, so returning only the
 * answers would fail it.
 */
export function buildAnsweredInput(
  input: unknown,
  questions: QuestionFormQuestion[],
  selections: QuestionSelections,
  otherTexts: QuestionOtherTexts,
): Record<string, unknown> {
  const base = typeof input === "object" && input !== null ? (input as Record<string, unknown>) : {};
  return { ...base, answers: buildQuestionFormAnswers(questions, selections, otherTexts) };
}

/**
 * Whether dismissing should be sent as an empty answer rather than a refusal.
 *
 * A question with no options and nothing required is a prompt, not a demand; closing it means "nothing to
 * say", and the conversation should carry on rather than record a denied tool call.
 */
export function shouldSubmitEmptyOnDismiss(questions: QuestionFormQuestion[]): boolean {
  return (
    questions.length > 0 &&
    questions.every((question) => question.allowEmpty && question.options.length === 0)
  );
}

/** The plan text of an ExitPlanMode proposal, or null when there is none to show. */
export function parsePlanText(input: unknown): string | null {
  if (typeof input !== "object" || input === null) return null;
  const plan = (input as Record<string, unknown>).plan;
  return typeof plan === "string" && plan.trim().length > 0 ? plan : null;
}

/** One question as the transcript records it: what was asked, and what it was answered with. */
export interface AnsweredQuestion {
  /** Short label for the question, the same one its tab carried while it was being answered. */
  header: string;
  question: string;
  /** The picked labels joined into one line, or the typed answer. Empty when the question was left blank. */
  answer: string;
}

/**
 * Read the answers out of a finished AskUserQuestion call.
 *
 * The answers are not in the arguments the agent proposed: the agent asked, and only afterwards did the
 * answer travel back inside an edited copy of those arguments, which the recording never sees. What the
 * recording keeps is the tool's own result line, `Your questions have been answered: "…"="…", "…"="…".`,
 * so that is where the answers are read from, with the arguments consulted first for the cases where the
 * host does hand back the edited input.
 *
 * Each answer is found by looking for the question it belongs to rather than by splitting the line on
 * punctuation: both the question and the answer are free text that may contain quotes and commas, and the
 * question text is already known here.
 *
 * Returns null when this is not an answered question — a call still waiting, a refused one, a result in a
 * wording this does not recognize — in which case the caller falls back to the plain tool card.
 */
export function parseAnsweredQuestions(
  input: unknown,
  output: string | undefined,
): AnsweredQuestion[] | null {
  const questions = parseQuestionFormQuestions(input);
  if (!questions) return null;
  const given = readGivenAnswers(input);
  const answered: AnsweredQuestion[] = [];
  for (const question of questions) {
    const answer = given?.[question.question] ?? extractAnswer(output, question.question);
    if (answer === undefined) continue;
    answered.push({ header: question.header, question: question.question, answer });
  }
  return answered.length > 0 ? answered : null;
}

/** The `answers` map, when the input on hand is the answered copy rather than the proposal. */
function readGivenAnswers(input: unknown): Record<string, string> | null {
  if (typeof input !== "object" || input === null) return null;
  const raw = (input as Record<string, unknown>).answers;
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) return null;
  const answers: Record<string, string> = {};
  for (const [key, value] of Object.entries(raw as Record<string, unknown>)) {
    if (typeof value === "string") answers[key] = value;
  }
  return Object.keys(answers).length > 0 ? answers : null;
}

/**
 * Pull one question's answer out of the result line.
 *
 * The answer runs to the quote that closes it, which is the one followed by the separator before the next
 * pair or by the end of the sentence. Anything else is a quote inside the answer itself.
 */
function extractAnswer(output: string | undefined, question: string): string | undefined {
  if (!output) return undefined;
  const marker = `"${question}"="`;
  const start = output.indexOf(marker);
  if (start < 0) return undefined;
  const from = start + marker.length;
  for (let i = from; i < output.length; i++) {
    if (output[i] !== '"') continue;
    const rest = output.slice(i + 1);
    if (rest.startsWith(", \"") || rest.startsWith(".") || rest === "") return output.slice(from, i);
  }
  return undefined;
}
