import { describe, expect, it } from "vitest";

import { parseShellContext, parseShellSubmission, shellErrorKey } from "./shellMode";

describe("parseShellSubmission", () => {
  it("takes everything after a leading ! as the command, trimmed", () => {
    expect(parseShellSubmission("!ls -la")).toEqual({ command: "ls -la" });
    expect(parseShellSubmission("  !  az login --tenant x ")).toEqual({ command: "az login --tenant x" });
    expect(parseShellSubmission("!echo 'a  b' | wc")).toEqual({ command: "echo 'a  b' | wc" });
  });

  it("reports a bare ! as an empty command rather than nothing", () => {
    expect(parseShellSubmission("!")).toEqual({ command: "" });
    expect(parseShellSubmission("!   ")).toEqual({ command: "" });
  });

  it("leaves prose alone, including a ! later in the text", () => {
    expect(parseShellSubmission("echo !x")).toBeNull();
    expect(parseShellSubmission("Hello! How are you")).toBeNull();
    expect(parseShellSubmission("")).toBeNull();
  });
});

describe("parseShellContext", () => {
  const tagged = (stderr: string) =>
    `<bash-input>git status</bash-input>\n<bash-stdout>clean\n</bash-stdout><bash-stderr>${stderr}</bash-stderr>`;

  it("reads the backend's context message back", () => {
    expect(parseShellContext(tagged(""))).toEqual({
      command: "git status", stdout: "clean\n", stderr: "", exitCode: 0, cancelled: false,
    });
  });

  it("takes the exit code and the cancel line off the end of stderr", () => {
    expect(parseShellContext(tagged("warn\nExit code 3"))).toMatchObject({ stderr: "warn", exitCode: 3 });
    expect(parseShellContext(tagged("Command cancelled by the user"))).toMatchObject({
      stderr: "", exitCode: undefined, cancelled: true,
    });
  });

  it("drops the truncation note and tolerates a missing stderr block", () => {
    const text = "<bash-input>yes</bash-input>\n<bash-stdout>[VelaTerm: earlier output truncated]\ntail</bash-stdout>";
    expect(parseShellContext(text)).toMatchObject({ command: "yes", stdout: "tail", stderr: "", exitCode: 0 });
    expect(parseShellContext("plain prose")).toBeNull();
    expect(parseShellContext("<bash-input>x</bash-input>")).toBeNull();
  });
});

describe("shellErrorKey", () => {
  it("maps the stable backend refusals and nothing else", () => {
    expect(shellErrorKey(new Error("chat_shell_running"))).toBe("chat.shell.alreadyRunning");
    expect(shellErrorKey("chat_shell_empty")).toBe("chat.shell.emptyCommand");
    expect(shellErrorKey("Failed to start the shell")).toBeNull();
  });
});
