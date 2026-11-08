---
name: vopen
description: >-
  Open a file or URL in the vlx-term center pane (mirrors the `vopen` command). Markdown opens in a WYSIWYG editor,
  images in the built-in image viewer, HTML pages in a live sandboxed preview, other files in the source-code editor
  (with automatic syntax highlighting by language), and http/https URLs in a built-in browser tab. Only use when the
  user explicitly invokes /vopen or $vopen; never auto-trigger. Available only inside vlx-term-hosted local sessions.
argument-hint: "<file|url>..."
disable-model-invocation: true
allowed-tools: Bash(vopen:*)
---

# /vopen

The user **explicitly invoked `/vopen` (Claude) or `$vopen` (Codex)** to request opening a file or URL in the
**vlx-term** center pane, with exactly the same effect
as typing `vopen <file>` in the terminal: it opens a document tab in the center pane, routed automatically by type
(markdown WYSIWYG / image viewer / code editor with syntax highlighting), while http/https URLs open a built-in
browser tab and Claude artifact links open in the system browser. The document tab sits alongside the terminal tabs and won't displace the current session. An HTML
page (`.html`, `.htm`) opens as a live preview tab that reloads on every save. To open a file in the panel beside
the conversation instead of a tab, use the `vartifact` command, which otherwise behaves the same.

User input:

$ARGUMENTS

## Step 1: Resolve the user input into concrete paths

- What the user gives may be a **literal path or URL**, or a **reference** (e.g. "the file I just edited," "the
  architecture doc") — resolve references into real file paths from the current conversation context; when unsure,
  ask first rather than guessing a path to open.
- Multiple items can be opened at once; resolve each one.
- **Always convert to an absolute path** before passing it to the command (the Bash tool's working directory isn't
  necessarily the "current directory" the user has in mind; absolute paths are the most reliable). Pass URLs
  (http/https) through as-is.
- The file must already exist; `vopen` only accepts regular files, not directories.

## Step 2: Run the command

Run `vopen` from PATH (each path/URL as a separate argument; mind escaping of spaces and other special characters):

```bash
vopen <absolute path or url>...
```

On success it prints a confirmation for each opened item; on failure (file doesn't exist or isn't a regular file)
it exits non-zero and reports which file on stderr.

## Step 3: Report back to the user

On success, give a one-line summary: "Opened in vlx-term: <file name>"; on failure, report the failed file and the
reason honestly.

## Notes

- Must run inside a **vlx-term-hosted local session**: it relies on the injected `VLX_*` environment variables and
  the `vopen` script on PATH; if missing it reports "not inside a vlx-term session" — in that case just tell the
  user this skill is only available inside vlx-term sessions, and don't try other ways to open it.
- Not available inside `claude-remote` remote sessions (a remote can't reach the local machine's hook port).
