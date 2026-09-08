---
name: vstat
description: >-
  You CAN see whether other vlx-term sessions are busy. This reports each session's current state —
  working, asking for permission, waiting for input — straight from the same status the sidebar shows.
  Use it when the user asks how a run or another session is doing ("is it done yet", "what are they up
  to", "how is the orchestration going") in any language, or after /vorch when you need the outcome.
  Read-only and instant; nothing is sent to any session. Available only inside vlx-term-hosted sessions.
argument-hint: "[<orch-id>|latest] [--wait] [--follow] [--timeout N] [--json]"
allowed-tools: Bash(vstat:*)
---

# vstat

`vstat` prints which vlx-term sessions are working, asking, or waiting. The answer comes from the same
authoritative status the sidebar displays, not from reading screens.

## Forms

```bash
vstat                     # every session known to vlx-term
vstat latest              # this session's most recent /vorch run
vstat <orch-id>           # one specific run
vstat latest --wait       # block until something in that run changes, then print
vstat latest --follow     # keep printing changes until every agent has stopped
vstat --json              # machine-readable output, with any of the above
```

Each line is `<id prefix>  <state>  <kind>  <name>  <how long in that state>`. A state of `unknown` means
the session has not reported yet, which usually means it is still starting.

## When to use which

- **Plain `vstat` or `vstat latest`** answers "how is it going" right now. Use this from a conversation.
- **`--wait`** is for scripts that want the next change without polling. It returns after one change or
  after `--timeout` seconds (default 60, at most 300).
- **`--follow`** is what an orchestration's coordinator tab runs. Do not run it from a conversation: it
  holds the turn until every agent has stopped.

## Reading the result

- `working`: the agent is in the middle of a turn.
- `asking`: it is blocked on a permission prompt and needs a person.
- `waiting`: it has stopped and is waiting for input. For a child session that was given one task, this
  means the task is finished, or the agent gave up; read its conversation with `vrefer` to tell which.

## Notes

- Must run inside a **vlx-term-hosted session**: it relies on the injected `VLX_*` environment variables
  and `vstat` on PATH; if missing it reports "not inside a VelaTerm session" and exits.
- `latest` and orchestration ids refer to runs proposed by this session with `/vorch`; other sessions'
  runs are visible only through the plain, unfiltered listing.
