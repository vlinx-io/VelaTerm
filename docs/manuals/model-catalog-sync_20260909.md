# Model Catalog Sync

Created: 2026-09-09

Updated: 2026-09-25 10:21

VelaTerm keeps the complete list of Claude models on velaterm.com, so new Claude models can appear in VelaTerm without an app update. This page explains where the model lists come from, how the Claude catalog stays up to date, and how to check its state.

## Where model lists come from

The same model list for each agent is used when you create a session or a child session, when you start a code audit, and when you organize a session into the knowledge base.

- **Claude:** the catalog from velaterm.com, combined with the models the local Claude CLI reports and any models configured in Claude's `settings.json`. The models the CLI reports never replace the complete catalog. A model listed in the catalog is not necessarily available to your account.
- **Codex:** the list that the local Codex app server reports for your account. The website catalog is not used for Codex.
- **OpenCode, Grok, Crush, Antigravity, Cursor, Pi, OMP and Kiro:** the list printed by the agent's own CLI.
- **Kimi:** a fixed set of models.
- **Copilot, Cline and Zoo:** no list; you can type a model name.

## How the Claude catalog is updated

When VelaTerm starts, it loads the last catalog saved in its database and checks velaterm.com. After that it checks every six hours, and every five minutes after a failed check. If the website cannot be reached or returns an invalid catalog, VelaTerm keeps the last valid catalog. When no catalog has been downloaded yet, VelaTerm uses the catalog bundled with the app.

The desktop app and browser clients connected to the same backend share one catalog, and an update reaches all connected clients.

## Checking the catalog

In a Claude session in the conversation view, hold Option (Alt on Windows and Linux) and click the model button below the message box. The model menu then also shows the catalog status:

- the source: "Website model catalog", "Cached model catalog" or "Bundled model catalog", followed by the catalog version;
- "Last checked: …";
- "Update failed. The previous catalog is still available." when the last check failed;
- a "Refresh" button that checks the website immediately.

## Diagnostics

- `VLX_MODEL_CATALOG_URL` replaces the download address. Only HTTPS addresses, or HTTP addresses on this machine (`localhost`, `127.0.0.1`, `[::1]`), are accepted.
- The downloaded catalog is cached in the application database under the `model-catalog.claude.v1` setting.
- Sync events are written to `logs/runtime-*.log` in the application data directory (`VLX_LOG_DIR` changes the directory). A successful check is logged at `INFO` and a failure at `WARN`. `VLX_MODEL_CATALOG_LOG_LEVEL` filters these events further; setting it to `ERROR` hides failures as well. The log contains neither credentials nor the catalog's content.

General log locations, levels, rotation and privacy are described in [Runtime logs and privacy](runtime-diagnostics_20260909.md).
