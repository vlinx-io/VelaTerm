---
name: vapp
description: >-
  Operate the VelaTerm app itself on the user's behalf: organize the sidebar (create collections and
  groups, import projects, rename, move, sort, archive or remove sessions, groups and projects), start or
  open sessions, open files, work with knowledge-base notes, or change app settings such as theme,
  language and font size. Use it whenever the user asks for something done in VelaTerm's own interface
  rather than in files or the shell, for example "put these sessions into groups by series" or "switch to
  the dark theme". The user confirms every action in VelaTerm before it runs. Available only inside
  VelaTerm-hosted local sessions.
argument-hint: "catalog [area] | session list [project] | session create-group|rename|move|sort ... | do '<JSON actions>' | status <id>"
allowed-tools: Bash(vapp:*)
---

# vapp

`vapp` lets you act in the VelaTerm application: its sidebar tree, files, knowledge base and settings.
You propose actions; VelaTerm shows them in this session's view, and the user confirms or skips them.
Nothing runs without that confirmation.

**Never work around `vapp`.** Do not read `vlx-local-link.json`, log in to the local web service, call the
app's internal commands, or write to `vlx-term.db`. If `vapp` cannot do something, tell the user.

## 1. Look at what is possible and the current state

```bash
vapp catalog                   # every area: its actions, and its state
vapp catalog projects          # one area: projects, files, knowledge, settings
vapp session list              # groups and sessions of this session's project, as a tree with ids
vapp session list "Course"     # another project or collection: id, id prefix, or exact name
vapp session list Course --json
```

The projects area lists collections and projects; `vapp session list` shows a project's groups and
sessions. Name sidebar items by full id, an id prefix of at least 6 characters, or an exact name when it
is unique. Prefer ids: session names are often long and similar. Other areas take ids exactly as listed.

## 2. One sidebar change

```bash
vapp session create-group <project> [--parent-group <group>] <name>
vapp session rename <item> <new name>
vapp session move <item> --group <group>          # into a group
vapp session move <item> --root                   # to the project root
vapp session move <item> --parent-session <session>
vapp session move <item> --order 3000             # reorder in place
vapp session sort <group|project|session> --by name
vapp session sort <group> --order id1,id2,id3
```

`sort --by name` orders numbers by value (lesson 2 before lesson 10). Sessions and subgroups are sorted
separately. With `--order`, items you leave out keep their relative order after the listed ones.

## 3. Several actions at once

Every `vapp session` change command is a request of its own. For more than one action, submit one
request with `vapp do`:

```bash
vapp do --summary "Group the lessons by series and switch to the dark theme" '[
  {"kind":"createGroup","projectId":"3f2a9c","name":"Figures","as":"figures"},
  {"kind":"rename","id":"8d41e0a7","name":"6. Zhang Heng"},
  {"kind":"move","id":"8d41e0a7","groupId":"$figures"},
  {"kind":"sort","groupId":"$figures","by":"name"},
  {"domain":"settings","kind":"set","setting":"theme","value":"dark"}
]'
```

- Each action has a `kind` and the fields `vapp catalog` lists for it. Give `domain` (`projects`,
  `files`, `knowledge`, `settings`) for everything outside the sidebar; sidebar actions may omit it.
- `"as": "name"` on a create action lets later actions in the same request use `"$name"` as its id.
- Actions run in the order given. Up to 500 per request; long arrays can be piped on stdin.
- `--summary` is shown to the user above the actions; write it in the user's language.
- The whole request is checked before anything is shown. If any action is invalid, nothing is submitted:
  the output lists each problem with the action's `index`, `field` and `reason` (`unknownItem`,
  `ambiguous`, `wrongKind`, `exists`, `cycle`, `notChild`, `builtIn`, `invalidPath`, `unsupported`...).
  Fix them and submit the whole request again.
- Consecutive sidebar changes appear on one card that the user applies or skips together. Actions that
  open a form or view, and imports where the user picks among several folders, get a card each.

## Waiting for the user

A submitting command waits until the user has decided on every action (15 minutes by default,
`--timeout` seconds to change it, `--no-wait` to return at once). Give the command a long enough timeout
in your tool call, or run it in the background. It then prints each action's `status`: `done` (with
`createdId` for new items), `failed` (with `error`) or `dismissed` (the user skipped it). An action that
needs an item the user skipped or that failed to be created fails too.

Exit code 2 means some actions were still undecided when the wait ended; check later with
`vapp status <id>`. Do not resubmit the same actions while they are pending.

## Rules

- Act only on what the user asked for. Ask about anything unclear (which project, which folder) instead
  of guessing.
- Some actions open VelaTerm's own form prefilled; `done` then means the form was opened, and the user
  finishes it there. Say so in your reply rather than claiming the change is complete.
- After sidebar changes are applied, list the tree again before planning further changes.
- Do not put passwords, tokens or other secrets in any action.
- Report the outcome plainly, including dismissed and failed actions.
