# Conversation View

Created: 2026-09-25 10:12

> The conversation view shows an agent session as a chat: messages, tool cards, and permission requests you answer with buttons. This chapter covers switching between the terminal view and the conversation view, sending messages while the agent works, the controls under the message box, rewinding, background tasks, usage limits, and reading long conversations.

## 1. What the conversation view is

Every agent session can run in the **Terminal view**, where the agent shows its own terminal interface. Five agents can also run in the **Conversation view**: Claude Code, Codex, OpenCode, Pi and OMP. The other agent types always use the terminal view.

In the conversation view, VelaTerm draws the conversation itself: your messages, the agent's answers and reasoning, one card per tool call, and cards for permission requests and questions. You can change the model, reasoning effort and permission mode from the controls under the message box without restarting the session.

Both views continue the same conversation. A session can move from one view to the other, and the history carries over.

## 2. Choosing a view

**Default view per agent.** Settings ▸ Agents ▸ "Default view" decides which view new sessions of that agent open in. The choices are "Conversation view" and "Terminal view"; the default is "Conversation view" for all five agents. Existing sessions keep the view they were created with.

**Per session.** The "New with launch args…" dialog has an "Opens in" option, so a single session can start in the other view.

**Switching an existing session.** The pane header has a switch button:

- In the terminal view, the robot button ("Conversation view") moves the session to the conversation view.
- In the conversation view, the terminal button ("Terminal view") moves it back. A short hint points at this button until you close the hint or use the button.

If the agent is working, VelaTerm asks first: "Switching views restarts the agent. The turn in progress will stop. Your conversation is kept." Choose "Switch" to continue. An idle session switches immediately.

## 3. Sending messages

The message box says "Message the agent, or use /commands, /skills and @files". Enter sends; Shift+Enter adds a line. With the cursor at the start or end of the text, ↑ and ↓ bring back your earlier messages from this conversation.

A new conversation shows "Type below to start the conversation." The agent process starts when you send the first message.

### While the agent is working

While a turn runs, the message box reads "Type a message; it will be sent when this turn ends". You have three ways to send:

| Action | macOS | Windows and Linux |
|--------|-------|-------------------|
| Queue: send when this turn ends | Enter | Enter |
| Steer: add the message to the running turn | ⌥Enter, or the "Steer" button | Alt+Enter, or the "Steer" button |
| Interrupt: stop the turn and send this message first | ⌘Enter | Ctrl+Enter |

Steering works with all five agents. When no turn is running, these key combinations simply send the message.

Queued messages appear under "Pending messages". Click a message to edit it, use its "Steer" button to add it to the running turn, or remove it with ×. Queued messages are sent in order as turns finish.

To stop the running turn, press Esc or click the stop button, which replaces the send button while the message box is empty ("Stop · Esc"). The conversation shows "Stopping the current turn…" and then "Current turn stopped".

## 4. Controls under the message box

The row under the message box holds controls for the next message. Which controls appear depends on the agent and model:

| Control | Agents | What it does |
|---------|--------|--------------|
| Model | All five | Pick the model. "Use <agent> default" follows the agent's own configuration |
| Thinking effort | All five | Pick the reasoning level offered by the selected model; Claude also offers "Off" |
| Collaboration mode | Codex, OpenCode | Codex: "Default" (work directly) or "Plan" (investigate and prepare a plan first). For OpenCode this control is labeled "Agent" and picks the OpenCode agent, such as build or plan |
| Permission mode | Claude, Codex, OpenCode, OMP | See the table below |
| Fast mode | Claude, on models that support it | Turn fast mode on or off |
| Speed | Codex, when several speeds are offered | Pick the service speed |
| Tone | Codex, on models that support it | "Default tone", "Neutral", "Friendly" or "Pragmatic" |
| MCP servers | Claude, Codex | Show configured MCP servers and their state; reconnect, disable or enable a server. For Codex, enabling or disabling a server changes your Codex user configuration, which other Codex conversations also use |
| Background tasks | Claude | See §9 |
| Account | Claude, Codex | Sign in again or sign out; see §12 |
| Codex reset credits | Codex | Show and use Codex rate-limit reset credits |

The Model, Thinking effort and Permission mode menus offer "Set as default", which makes the current choice the default for new conversations of that agent. An effort default belongs to the model it was chosen for.

Settings ▸ Conversation view ▸ "Composer toolbar" decides which controls sit beside the message box and in which order. Model, Thinking effort, Collaboration mode and Permission mode are on by default. Controls that are off, or that do not fit, move to the **More** menu, so every supported control stays reachable. MCP servers and Background tasks are disabled while the agent process is not running ("The agent process is not running. Send a message to start it.").

Claude and Codex conversations also show a small context meter next to the send button once the agent reports its first figures. Its color changes as the context fills up or a rate limit approaches. Hover it to see the context usage ("Context: … of … tokens"), the session cost when the agent reports one, and rate-limit warnings.

### Permission modes

| Agent | Modes |
|-------|-------|
| Claude Code | "Plan Mode", "Always Ask", "Accept File Edits", "Auto mode", "Bypass" |
| Codex | "Read Only", "Auto mode", "Full Access" |
| OpenCode | "Always Ask", "Bypass" |
| OMP | "Always Ask", "Bypass" |
| Pi | No permission control; Pi runs tools without asking |

A change to a Codex session applies to the next turn, and the menu marks it accordingly. Switching a running Claude session to "Bypass" requires a restart; VelaTerm asks "Restart to enable Bypass?" and keeps the conversation history. "Bypass" and "Full Access" skip all confirmations, so use them with care.

## 5. Answering the agent

When the agent needs you, a card appears at the end of the conversation and the session's status dot turns yellow:

- **Permission request**: "<tool> wants to run", with "Allow" and "Deny". Some requests also offer standing rules, such as "Allow … for this session" or "Always allow …".
- **Question**: "The agent has a question". Pick or type an answer, then "Next" or "Submit"; "Dismiss" leaves it unanswered.
- **Plan approval**: "Plan ready for approval", with "Approve and run" and "Reject".
- **MCP server input**: "<server> is asking for input", with "Submit", "Decline" and "Cancel".

## 6. Images

Paste or drop images into the message box. A message can carry up to 4 images of up to 5 MB each; the message box explains why an image was left out. Thumbnails appear above the text, each with a remove button. An image on its own can be sent without text. Right-click an image in the conversation to "View original image", "Copy image" or "Save image".

## 7. Commands and completion

Type `/` to list commands and `@` to insert a file path. Tab or Enter inserts the highlighted suggestion; Esc closes the list. For Claude and Codex, the list includes the agent's own commands and skills.

VelaTerm handles these commands itself:

| Command | Agents | What happens |
|---------|--------|--------------|
| `/clear` (also `/new`) | All five | Archives this session and replaces it, in the same pane, with a new empty session that has the same settings |
| `/rewind` | Claude, Codex, OpenCode | Opens the rewind menu on your latest message (see §8) |
| `/compact` | Codex, OpenCode, Pi, OMP | Summarizes the conversation to free up context |
| `/review` | Codex | Asks Codex to review code; accepts `branch <name>`, `commit <sha>` or instructions |
| `/undo`, `/redo` | OpenCode | Reverts the last message and its file changes, or restores what the last undo reverted |
| `/share`, `/unshare` | OpenCode | Creates or removes a link that shares the conversation |

## 8. Rewinding and editing

Your messages have a "Rewind from here" button. What it can restore depends on the agent:

| Agent | Choices |
|-------|---------|
| Claude Code | "Rewind conversation", "Restore files", "Rewind conversation and restore files" |
| Codex | "Rewind conversation" |
| OpenCode | "Rewind conversation and restore files" |
| Pi, OMP | Not available |

Rewinding removes the chosen message and everything after it. It is available only while the agent process is running and idle, with no queued messages or open permission requests. File restoration shows how many files will change before you confirm. Rewinding cannot be undone.

Claude and Codex messages also have an "Edit" button. Change the text or images, choose "Review and resend", and confirm with "Delete and resend". The original message and everything after it are permanently deleted, and your edited message is sent from that point. File changes are not restored.

## 9. Background tasks (Claude)

Claude can run subagents and commands in the background. The **Background tasks** control lists them with their state ("Running", "Completed", "Failed", "Stopped"). Use "Stop" to end a task, or open it in its own tab to see its elapsed time, token count, tool calls, prompt and result. While a turn is running, "Move the running work to the background" sends the current work to the background so you can continue the conversation.

While background tasks run, the session keeps its "working" status.

## 10. Usage limits and automatic continuation

When a Claude or Codex conversation stops because a five-hour or weekly usage limit was reached, VelaTerm continues the task once the limit resets. This is on by default; switch off Settings ▸ Behavior ▸ "Continue after limit resets" to disable it.

With the setting on, the conversation shows when it will continue, for example "5-hour usage limit reached. The task will continue automatically at 14:30.", with a "Cancel" button. The wait survives an application restart. It ends without sending anything when you send a message yourself, rewind, cancel it, turn the setting off, archive or delete the session, or move the session to the terminal view. If the reset time is unknown, or the limit is reached again after continuing, the conversation says so and does not continue on its own.

## 11. Reading long conversations

- The latest part of the conversation loads first. Scroll up, or choose "Load earlier messages", to read further back.
- Consecutive tool calls fold into one line ("N tool calls"); click it to see each call.
- Each answer can hide its intermediate steps ("Hide steps" / "Show N steps"). The pane header has "Hide all steps" / "Show all steps".
- The "Search…" button in the pane header, or the Find in terminal shortcut (⌘F / Ctrl+Alt+F), searches the conversation.
- After scrolling up, "Back to the latest message" returns to the end.
- Your own messages have a copy button. Text in any message can be selected and copied.

Archived conversations open read-only in the knowledge base; see [Interface & Session Management](interface-and-sessions_20260709_2041.md) §8.

## 12. Signing in and out (Claude and Codex)

The **Account** control ("Claude account" or "Codex account") is off in the toolbar by default. Open it from **More**, or turn it on in Settings ▸ Conversation view ▸ "Composer toolbar". It offers "Sign in again" and "Sign out". Account changes wait until the current turn finishes; Claude also waits for background tasks and permission requests.

**Codex.** When a Codex request reports that the sign-in is no longer valid, the conversation offers "Sign in again". Open the authorization page and enter the code shown in the conversation. The view updates when sign-in finishes, and you can continue the same conversation. Device code sign-in must be enabled for your ChatGPT account or workspace, and your Codex CLI must support it. "Cancel" stops a pending sign-in.

**Claude.** "Sign in again" shows an authorization link. Sign in on that page, paste the full code it shows (including the part after `#`) into "Authorization code", and choose "Submit code". The code is not sent as a chat message. This requires a Claude CLI version that supports account authorization.

**Sign out** asks for confirmation. It clears the account credentials on the connected computer, so other sessions that use the same credentials are affected. Conversation history is kept. After signing out, the conversation offers "Sign in" to continue. Signing out of Claude does not change configured API keys.

## 13. Shell commands with `!`

Type `!` followed by a command, for example `! az login --tenant <id>`, to run the command in the session's shell instead of sending it to the agent. This works for all five agents.

- The command runs in the agent's working directory with the agent's environment. The text after `!` is passed to the shell as typed.
- A "Shell command" row shows the command, its live output and a "Cancel" button. When it ends, the row shows the exit code or "Cancelled". Long output keeps only its last part, and the row says so.
- After the command ends, the agent receives the command and its output and can react to it.
- One shell command runs per conversation at a time. Shell commands cannot carry images and do not accept interactive input. There is no time limit, so a command that waits for a browser sign-in keeps running until it finishes or you cancel it.

## 14. When the agent process stops

- Closing the conversation's tab releases the agent process: an idle agent stops right away, and a working agent stops after its current turn. The next message you send starts it again and continues the same conversation.
- "Kill Process" in the session's context menu ends a running agent after confirmation. The conversation history is kept.
- If the agent exits on its own, the conversation shows "The agent stopped (exit N)".
- If the agent is not installed, the pane shows "<Agent> is not installed" with "Install now", "Install docs" and "I'll do it myself". "Install now" switches the session to the terminal view and runs the install there; see [AI Agent Sessions](ai-agent-sessions_20260709_2041.md) §7.
