# Claude model catalogue: the website catalogue merged with the installed CLI

Created: 2026-09-22
Updated: 2026-09-23 (merge instead of replace, identifier folding, `availableModels`, session scope)

The model chip of a Claude session and the model menus of the new-session dialog start from VelaTerm's model catalogue (the website catalogue, or the bundled table when that is not available) and merge in the models the installed Claude Code CLI reports. The CLI's list is a shortlist of recommendations, not the set of models it runs, so it never replaces the catalogue: every model the catalogue lists stays selectable, a CLI row updates the capabilities of the entry it names, and a model the catalogue does not list yet is appended. A model that Anthropic releases therefore appears in VelaTerm as soon as the user's Claude Code knows it, in every existing session as well as in new ones, without a VelaTerm update and without anyone editing a model table or the website catalogue.

## How the list is built

One function builds the list for every consumer (the session's model chip, the new-session dialog, the agent model list, the knowledge base runner and the code audit model list). The steps, in order:

1. **The base.** The website catalogue when it is present, see [Website model catalogue sync](model-catalog-sync_20260909.md); otherwise the bundled table shipped with VelaTerm, filtered to what the installed CLI version accepts.
2. **The CLI rows are merged in.** Each row is keyed by the model it resolves to, folded onto the catalogue's spelling (see [Identifier folding](#identifier-folding)):
   - A row whose model the list already has **updates that entry**: effort levels (when the row names any), fast-mode support and the default mark come from the CLI. The entry keeps the catalogue's identifier, label and context window; the CLI's description and display name are used only where the catalogue has none.
   - A row whose model the list does not have is **appended**, with a label derived from its identifier (for example Opus 5.5 on release day, before the website catalogue lists it).
   - A row the CLI marks as **disabled removes** its model from the list, including a catalogue entry.
   - The CLI's `default` row is not shown as a model. The model it resolves to is marked as the current default and appended when nothing else lists it. The **Use Claude default** entry of the chip names it in its hint, for example "Uses the model selected by the agent's configuration (currently Opus 5.5)".
3. **Additions** that running conversations reported (see [Session scope](#session-scope)) are appended when nothing else lists them. They never change or remove an entry.
4. **Identifiers from `settings.json`.** Identifiers configured under `env` in `~/.claude/settings.json` (`ANTHROPIC_MODEL` and the other model keys) are appended as before, unless the list already has them.
5. **The `availableModels` filter**, when Claude Code's `availableModels` setting is present, see [availableModels](#availablemodels).

Which CLI rows step 2 uses depends on the session, see [Session scope](#session-scope).

## Where the CLI rows come from

- **The neutral probe.** VelaTerm asks the configured Claude binary headlessly (`claude -p --input-format stream-json --output-format stream-json --verbose --settings {"disableAllHooks":true} --strict-mcp-config --no-session-persistence`, control requests `initialize` and `list_models` only, stdin closed afterwards). The probe sends no user message and spends no account usage. Hooks and MCP servers are switched off and no transcript is written, like the knowledge base runner's headless launch. The probe runs in a private directory `model-probe` below the app data directory (restricted to the owner on Unix, re-applied on every probe), never in a shared directory such as `/tmp`, because Claude Code reads the project settings of its working directory; the private, empty directory keeps any project's settings from shaping the answer. When that directory cannot be created, the probe counts as failed. The probe runs off the main thread, is not a VelaTerm session (VelaTerm's session identity and hook settings are removed from its environment), and tolerates the system lines the CLI prints while starting. On Unix the CLI runs in its own process group; when the probe times out, or the CLI keeps running after it has answered, the whole process group is killed, so nothing the CLI started survives it.
- **A running conversation's own list.** A Claude conversation that is running reports its models over the stream-json control protocol (`list_models`). Its answer may reflect that conversation's project settings.

## Session scope

- **A running session** merges its own conversation's `list_models` answer into the catalogue, in full: its disabled rows remove entries and its default is marked, for that session's menu only. The neutral probe's rows and the additions are appended only where they name a model the list does not have yet. Building this menu never starts a probe.
- **Every other consumer** (a session whose conversation is not running, the new-session dialog, the agent model list, the knowledge base runner, the code audit model list) merges the neutral probe's rows and appends the additions.
- **Additions.** A running conversation's answer never replaces the probe's list for other sessions. Instead, the identifiers it names are remembered for its binary as additions: rows that are not disabled, not the `default` row (whose target a project can change) and not already in the probe's list. An addition only ever appends a model; it never changes an entry's capabilities and never brings back a model a disabled row removed. It stays remembered until the probe lists that model itself, the CLI version changes, or Refresh is pressed. Additions are only for menus: they never decide how an alias is resolved at launch, because a conversation's answer can depend on its project settings. This way a new model reported by one conversation appears in every menu, while one session's narrow answer never shrinks another session's menu.

A Claude chat pane reads the list when the pane is created, when its own conversation reports models, and whenever the catalogue changes (the start-up probe finishes, Refresh is pressed, or a conversation reports a model that was not remembered yet).

## Identifier folding

The CLI and the catalogue sometimes spell the same model differently. One folding rule is used by the merge, by the alias chokepoint below and by the chip that shows a stored selection, so a stored selection in either spelling resolves to the same menu row. Folding compares against the catalogue plus the whole bundled table, so a spelling folds the same way even when the website catalogue does not list that model yet or the version filter hides its bundled entry.

- An identifier the catalogue lists is kept as it is.
- A **dated identifier** folds onto the undated entry when the catalogue lists one: `claude-haiku-4-5-20251001` becomes `claude-haiku-4-5`.
- A **`[1m]` suffix on a natively 1M model** folds away when the catalogue lists the plain identifier with a 1M context window and no separate suffixed entry: `claude-opus-5-5[1m]` becomes `claude-opus-5-5`, `claude-fable-5-1[1m]` becomes `claude-fable-5-1`. A website entry without a context window falls back to what the bundled table says about the same identifier.
- A **`[1m]` suffix on a standard-context model** asks for a different context window, so that identifier stays a separate entry: `claude-opus-4-6[1m]` and `claude-sonnet-5[1m]` are not folded.
- Identifiers of any other shape, and models neither the catalogue nor the bundled table knows, keep their spelling.

The bundled table lists Opus 5.5 as a single, natively 1M entry `claude-opus-5-5` (label **Opus 5.5**), offered from Claude Code 2.1.280 on.

## Aliases and saved preferences

Short names and old spellings are resolved through one chokepoint that the chat engine uses at launch, when the model is changed, and when the CLI reports its model at start; the chip of a session whose conversation is not running folds only the spelling of the stored selection (`claude-opus-5-5[1m]` shows as `claude-opus-5-5`, a dated id as its undated entry) and leaves aliases such as `opus` as the user wrote them, because that value is saved again on the next send. With the neutral probe's `value` to `resolvedModel` pairs present, the CLI decides: `opus[1m]` becomes whatever the installed CLI resolves it to, an identifier the CLI offers is not rewritten, a static alias applies when its target is a model the CLI offers, any other short name such as `opus` is left to the CLI once the CLI offers another generation, and a retired full spelling such as `claude-opus-5[1m]` keeps its old rewrite to `claude-opus-5`. Without CLI data the static alias table applies; it resolves `opus`, `opus[1m]` and `claude-opus-5-5[1m]` to `claude-opus-5-5`. Either way the result is folded as described above, so for Claude Code 2.1.280 `opus[1m]` ends up as `claude-opus-5-5`. Saved preferences with old spellings keep working through the same path.

Offering and accepting differ on purpose. A knowledge-base or security job saved with a model the menu no longer names still validates: accepted are the merged list before the `availableModels` filter, the whole bundled table, and the stored identifier itself when it is a well-formed Claude model id (`claude-` followed by lowercase letters, digits, dots and hyphens, optionally `[1m]`, at most 64 characters). The CLI decides at start whether it still runs that model.

## availableModels

Claude Code's `availableModels` setting restricts which models a user may pick. When it is set, VelaTerm filters the menus to what it allows, so it does not offer catalogue entries the CLI would refuse. Without the setting, or with an empty list, nothing is filtered.

Where it is read:

- The file-based managed settings (`/Library/Application Support/ClaudeCode/managed-settings.json` on macOS, `C:\Program Files\ClaudeCode\managed-settings.json` on Windows, `/etc/claude-code/managed-settings.json` elsewhere). When that file has the setting, its list applies alone.
- Otherwise the user's `settings.json` and `settings.local.json` in the Claude configuration directory (`~/.claude`, or `CLAUDE_CONFIG_DIR`); the lists of both files are combined without duplicates.
- Not read: the macOS configuration profile, the Windows registry, server-managed settings, `managed-settings.d` and project settings.

How entries match, following Claude Code's model configuration documentation (the matching was taken from that documentation and has not been checked against the installed CLI binary):

- A family alias such as `opus`, `fable`, `sonnet` or `haiku` allows every model of that family.
- Any other entry is a version prefix or a full identifier. It allows itself and every identifier that extends it by a further segment: `claude-fable-5` allows Fable 5 and Fable 5.1, `claude-haiku-4-5` also its dated form.
- A specific entry of a family switches that family's alias off: with `fable` and `claude-fable-5` in the list, only the models `claude-fable-5` allows are offered from the Fable family.
- A `[1m]` suffix is ignored on both sides.

The list `["fable", "claude-fable-5", "opus"]` therefore offers every Opus model, Fable 5 and Fable 5.1, and no Sonnet or Haiku model. The filter also applies to the identifiers from `settings.json`. It narrows only what the menus offer, never the set a saved selection is validated against.

## Cache, version check and refresh

The probe result is cached per binary in the app database together with the CLI version string and the check time. The cache is reused until one of these happens:

- `claude --version` reports a different version. The version is read at most once per app run (and again on Refresh). At app start VelaTerm loads the stored cache and checks the default Claude binary once in the background, so an update of Claude Code followed by a VelaTerm restart re-probes at start or on first use and the menus show the new list. When the version output cannot be read at all, a list probed during the current run is kept for the rest of the run and a list stored by an earlier run is probed afresh.
- **Refresh** in the model menu is pressed. Refresh re-reads the version and probes the CLI first; only when the probe fails does it fall back to refreshing the website catalogue.

Only a probe's own answer counts as the cached list: when only running conversations have reported so far, the probe is still run. A successful probe keeps the additions it does not list itself, as long as they were reported by the same CLI version; Refresh starts over.

Only one probe per binary runs at a time; a second reader waits for the running probe instead of starting another process. After a failed probe the same binary is not asked again for a backoff period, the same as for the website catalogue; Refresh ignores the backoff.

The cache entry is treated as a protected setting: remote clients cannot read or write it directly. On the public share surface the catalogue is answered from the cache only, so a visitor can never start a process on the host; the backend decides this, not the client.

## Existing sessions after a Claude Code update

- **A session whose conversation is not running** gets the new model from the fresh probe after VelaTerm restarts (the version changed, so the old cache is invalid), or earlier, as soon as a running conversation reports it as an addition. No new session is needed.
- **A conversation resumed after the restart** reports its list itself through `list_models`; the chip re-reads it on the models event.
- **A conversation still running on the old binary** (Claude Code updated in place, VelaTerm not restarted) merges its own list into the catalogue. The catalogue's entries stay listed; whether that process runs a model it does not report is decided by the CLI when the model is selected.

## Labels

Catalogue entries keep the catalogue's label. An appended model gets a label that names the model generation, derived from the identifier: `claude-opus-6` is shown as **Opus 6**, `claude-fable-5-1` as **Fable 5.1**, `claude-sonnet-4-6` as **Sonnet 4.6**, `claude-haiku-4-5` as **Haiku 4.5**, and an identifier that keeps a `[1m]` suffix gets **1M** appended. An identifier of another shape falls back to the CLI's display name, then to the identifier. Identifiers ending in `[1m]` are marked as large context; a natively 1M model folded onto its plain identifier is not.

## Status line

The status line at the bottom of the model menu shows where the list comes from:

- **Installed Claude CLI** with the CLI version and the check time, when CLI data for the binary is cached (from a probe or from a running conversation's additions).
- **Website model catalog**, **Cached model catalog** or **Bundled model catalog** with the catalogue revision, as before, when no CLI data is available.
- **Update failed. The previous catalog is still available.** when the last probe or website check failed; the previous list stays in place.

## Failure modes

A missing binary, a timeout, a non-zero exit, malformed output, a signed-out CLI or an empty list never block the UI: the menu shows the catalogue without the CLI's rows, the failure is shown in the status line, and the binary is left alone for the backoff period. Codex and the other agents keep their own catalogue mechanisms; this page concerns Claude only.

## Diagnostics

The client RPC `model_catalog_status` returns `source` (`cli`, `website`, `cache` or `bundled`), `cliVersion` (with `source: "cli"`), `revision`, `checkedAt`, `error` and `refreshing`; `model_catalog_refresh` triggers the refresh described above. The probe cache is the `model-catalog.claude.cli.v1` entry of `app_settings` in the app database, a map from binary hash to snapshot: `version`, `checkedAt`, `origin` (`probe` once a probe succeeded, `live` while only running conversations have reported), `models` (the probe's rows), `live` (the remembered additions, omitted when empty) and `liveVersion` (the CLI version that reported them). A cache written by the earlier rule, where a conversation's whole answer was stored as a `live` snapshot's `models`, is dropped when loaded and derived again.

## What the tests cover

The Rust tests in `src-tauri/src/agent/cli_model_catalog.rs`, `claude_models.rs`, `launch_models.rs`, `web/dispatch.rs` and `web/share_policy.rs` and the frontend tests `ModelCatalogStatus.test.tsx` and `ChatPane.settings.test.tsx` cover:

- the merge as a table over the source combinations (catalogue only; catalogue and CLI with a new identifier; a CLI row updating a catalogue entry's capabilities while the catalogue keeps its identifier, label and description; a disabled row; an empty CLI list; the website unavailable, so bundled table and CLI; bundled only), the settings identifiers appended once, the default mark, a default resolving to a model nothing else lists, and the models Opus 4.8, Sonnet 5 1M, Opus 4.7, Opus 4.6 and Fable 5 staying selectable next to the CLI's shortlist;
- additions appending without changing capabilities or reviving a model a disabled row removed;
- identifier folding (natively 1M, dated, explicit `[1m]` variants kept, unknown identifiers kept, a catalogue with its own suffixed entry, a website entry without a context window), stored selections in either spelling resolving to the same menu row with and without CLI pairs, and the chip of a resting session showing the folded stored selection;
- alias resolution with and without CLI data, including rows without `resolvedModel` and retired full spellings;
- the `availableModels` matching (family aliases, prefixes and full identifiers, a specific entry switching the alias off, `[1m]` ignored, no setting) and reading it from real settings files (managed settings alone, user files combined without duplicates, an empty list, malformed JSON);
- the probe: parsing the recorded answer of Claude Code 2.1.280 including hook lines, its command line through a logging fake binary (hooks, MCP servers and session persistence off, the private working directory and its owner-only permissions, no session identity in the environment), malformed output, a missing binary, a timeout, and on Unix the timeout killing a grandchild the fake CLI started;
- the backoff and Refresh ignoring it, single flight per binary, cache reuse for the same version and replacement after a version change, the unreadable version case;
- session scope through the real engine path: a running conversation's answer shaping its own menu (catalogue kept, its disabled row applied, its default marked, no probe started), another session of the same binary served by the neutral probe with the live answer's new model appended and without that session's narrowing or default, and a live answer only adding to the cached probe snapshot;
- the wider accepted set for saved selections, including well-formed identifiers the list no longer names and the rejection of malformed ones;
- an open pane re-reading the list on a catalogue change, the label formatter, the protected settings key, the cache-only answer on the public share surface, the status line texts and the named default hint.

Not covered by an automated test: the background check at app start; the failure path when the private working directory cannot be created; the live refresh of an open chat pane when another conversation reports a new model; the step that applies the `availableModels` list read from the machine's settings to the menus (tests never read the machine's settings, so only the matching and the file reader are tested); the `availableModels` matching against the installed CLI binary (it follows the documentation); the process tree kill on Windows; and a probe against a real installed Claude Code (an ignored test, run by hand only). The probe duration of a few seconds is a measured observation, not a guarantee.
