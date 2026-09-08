//! Tests for reading an AskUserQuestion proposal, writing the answers back into it, and reading them out
//! of a finished call.

import { describe, expect, it } from "vitest";

import {
  areQuestionsAnswered,
  parseAnsweredQuestions,
  buildAnsweredInput,
  buildQuestionFormAnswers,
  isQuestionAnswered,
  parsePlanText,
  parseQuestionFormQuestions,
  questionShowsTextInput,
  shouldSubmitEmptyOnDismiss,
  type QuestionFormQuestion,
} from "./questionForm";

const input = {
  questions: [
    {
      question: "Which database?",
      header: "Database",
      multiSelect: false,
      options: [
        { label: "Postgres", description: "Relational" },
        { label: "SQLite", description: "One file" },
      ],
    },
    {
      question: "Which features?",
      header: "Features",
      multiSelect: true,
      options: [{ label: "Search" }, { label: "Export" }],
    },
  ],
};

const parsed = () => parseQuestionFormQuestions(input) as QuestionFormQuestion[];
const pick = (...indices: number[]) => new Set(indices);

describe("parseQuestionFormQuestions", () => {
  it("reads questions, their options, and whether several answers are allowed", () => {
    const questions = parsed();
    expect(questions.map((q) => q.header)).toEqual(["Database", "Features"]);
    expect(questions[0].multiSelect).toBe(false);
    expect(questions[1].multiSelect).toBe(true);
    expect(questions[0].options[0]).toEqual({ label: "Postgres", description: "Relational" });
    expect(questions[1].options[0].description).toBeUndefined();
  });

  it("offers a typed answer unless the request rules it out", () => {
    expect(parsed()[0].allowOther).toBe(true);
    const strict = parseQuestionFormQuestions({
      questions: [{ ...input.questions[0], allowOther: false }],
    });
    expect(strict?.[0].allowOther).toBe(false);
    expect(questionShowsTextInput(strict![0])).toBe(false);
  });

  it("returns null for anything that is not a question form", () => {
    expect(parseQuestionFormQuestions(null)).toBeNull();
    expect(parseQuestionFormQuestions({ file_path: "/x" })).toBeNull();
    expect(parseQuestionFormQuestions({ questions: [] })).toBeNull();
    expect(parseQuestionFormQuestions({ questions: [{ question: "?" }] })).toBeNull();
    expect(parseQuestionFormQuestions({ questions: [{ ...input.questions[0], options: [{}] }] })).toBeNull();
  });
});

describe("isQuestionAnswered", () => {
  it("counts a pick or typed text, and nothing else", () => {
    const questions = parsed();
    expect(isQuestionAnswered(questions[0], 0, {}, {})).toBe(false);
    expect(isQuestionAnswered(questions[0], 0, { 0: pick(1) }, {})).toBe(true);
    expect(isQuestionAnswered(questions[0], 0, {}, { 0: "  " })).toBe(false);
    expect(isQuestionAnswered(questions[0], 0, {}, { 0: "MySQL" })).toBe(true);
  });

  it("treats a blank as an answer only when the question allows one", () => {
    const [open] = parseQuestionFormQuestions({
      questions: [{ question: "Anything else?", header: "Notes", options: [], allowEmpty: true }],
    })!;
    expect(isQuestionAnswered(open, 0, {}, {})).toBe(true);
  });
});

describe("areQuestionsAnswered", () => {
  it("waits for every question, not just the one on screen", () => {
    const questions = parsed();
    expect(areQuestionsAnswered(questions, { 0: pick(0) }, {})).toBe(false);
    expect(areQuestionsAnswered(questions, { 0: pick(0), 1: pick(1) }, {})).toBe(true);
    expect(areQuestionsAnswered(null, {}, {})).toBe(false);
  });
});

describe("buildQuestionFormAnswers", () => {
  it("keys answers by the question text the tool asked with", () => {
    const answers = buildQuestionFormAnswers(parsed(), { 0: pick(0), 1: pick(0, 1) }, {});
    expect(answers).toEqual({
      "Which database?": "Postgres",
      "Which features?": "Search, Export",
    });
  });

  it("prefers what was typed over what was picked", () => {
    const answers = buildQuestionFormAnswers(parsed(), { 0: pick(0), 1: pick(0) }, { 0: " MySQL " });
    expect(answers["Which database?"]).toBe("MySQL");
  });

  it("leaves an unanswered question out rather than answering it blank", () => {
    expect(buildQuestionFormAnswers(parsed(), { 0: pick(1) }, {})).toEqual({
      "Which database?": "SQLite",
    });
  });
});

describe("buildAnsweredInput", () => {
  it("keeps the proposal and adds the answers, so the tool's own schema still holds", () => {
    const updated = buildAnsweredInput(input, parsed(), { 0: pick(1), 1: pick(0) }, {});
    expect(updated.questions).toBe(input.questions);
    expect(updated.answers).toEqual({
      "Which database?": "SQLite",
      "Which features?": "Search",
    });
  });
});

describe("shouldSubmitEmptyOnDismiss", () => {
  it("closes a free-text prompt as an empty answer, but refuses a real choice", () => {
    const open = parseQuestionFormQuestions({
      questions: [{ question: "Anything else?", header: "Notes", options: [], allowEmpty: true }],
    })!;
    expect(shouldSubmitEmptyOnDismiss(open)).toBe(true);
    expect(shouldSubmitEmptyOnDismiss(parsed())).toBe(false);
  });
});

describe("parsePlanText", () => {
  it("reads the plan a proposal carries, and nothing from one without", () => {
    expect(parsePlanText({ plan: "## Steps\n1. Do it" })).toBe("## Steps\n1. Do it");
    expect(parsePlanText({ plan: "   " })).toBeNull();
    expect(parsePlanText({})).toBeNull();
    expect(parsePlanText(null)).toBeNull();
  });
});

describe("parseAnsweredQuestions", () => {
  const result = (body: string) => `Your questions have been answered: ${body} You can now continue with these answers in mind.`;

  it("reads each answer out of the result line", () => {
    const answered = parseAnsweredQuestions(
      input,
      result('"Which database?"="SQLite", "Which features?"="Search, Sync".'),
    );
    expect(answered).toEqual([
      { header: "Database", question: "Which database?", answer: "SQLite" },
      { header: "Features", question: "Which features?", answer: "Search, Sync" },
    ]);
  });

  it("reads an answer that carries quotes and commas of its own", () => {
    const answered = parseAnsweredQuestions(
      { questions: [input.questions[0]] },
      result('"Which database?"="the one they call "small", plainly".'),
    );
    expect(answered?.[0].answer).toBe('the one they call "small", plainly');
  });

  it("keeps a question that was left blank", () => {
    const answered = parseAnsweredQuestions(input, result('"Which database?"="".'));
    expect(answered).toEqual([{ header: "Database", question: "Which database?", answer: "" }]);
  });

  it("prefers the answers the input carries", () => {
    const answered = parseAnsweredQuestions(
      { ...input, answers: { "Which database?": "Postgres" } },
      result('"Which database?"="SQLite".'),
    );
    expect(answered?.[0].answer).toBe("Postgres");
  });

  it("reads nothing from a call that has not been answered", () => {
    expect(parseAnsweredQuestions(input, undefined)).toBeNull();
    expect(parseAnsweredQuestions(input, "Dismissed by user")).toBeNull();
    expect(parseAnsweredQuestions({ plan: "not a question" }, result('"x"="y".'))).toBeNull();
  });
});
