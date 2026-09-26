# Document & Browser Tabs

Created: 2026-07-09 20:41

Updated: 2026-09-25 10:21

> Center-pane tabs are not only terminals. This chapter covers two other kinds: document tabs (the built-in Markdown and code editor) and browser tabs (embedded web pages). They share the tab bar with session tabs, and opening a session never replaces them.

## 1. Document tabs

### 1.1 Opening

A document tab opens when you:

- run `vopen <file>` in any session (inside Claude Code the `/vopen` skill does the same once "Vela Skills" is installed in Settings ▸ General);
- double-click a file in the right panel's Files tab, or choose "Open in Editor";
- open a note from the knowledge base (see [Local Knowledge Bases](knowledge-notebooks_20260910.md));
- click "New document" at the right end of the tab bar.

The view is chosen by file type:

| Type | View |
|------|------|
| md / markdown / mdx | Markdown editor with "Visual", "Source" and "Compare" modes; opens in Visual |
| Images (png, jpg, jpeg, gif, webp, svg, bmp, ico, avif) | Image viewer with "Fit" and "1:1" |
| Everything else | Code editor with syntax highlighting chosen from the file name |

![A Markdown document opened with vopen](../assets/manuals/doc-tab.png)

### 1.2 The Markdown editor

- **Visual** is a what-you-see-is-what-you-get editor. Type `/` for the insert menu (headings, lists, task lists, quotes, code blocks, tables, math, images and dividers). Fenced `mermaid` code blocks are rendered as diagrams below the block.
- **Source** shows the file text exactly as stored, including any YAML front matter, with Markdown syntax highlighting.
- **Compare** shows Visual and Source side by side and keeps them in sync while you edit either one.
- ⌘/ (Ctrl+/ on Windows and Linux) switches between Visual and Source. Switching modes without editing does not change the file.
- Pasting or dropping an image into a saved document stores the image in an `assets` folder next to the document and inserts a relative link to it.

### 1.3 Editing and saving

- ⌘S or the "Save" button writes the file to disk. A dot on the tab marks unsaved changes.
- The active document checks the disk every two seconds. If another program changes the file, a banner offers "Reload" and "Ignore"; when you also have unsaved edits, the reload button reads "Reload (discard my changes)". If the file changed on disk after you started editing, saving asks for confirmation in a "Save Conflict" dialog before it overwrites the file.
- Closing a tab with unsaved changes asks you to choose "Save & Close", "Close Without Saving" or "Cancel".
- ⌘F opens the search bar with find, replace, replace all and "Match case"; it works the same way in every mode.
- The "Sidebar" button shows the document's "Outline" and a "File tree" of its folder; click a file there to open it. The sidebar can be resized.
- Files larger than 10 MB open read-only and show only their first 10 MB, so that saving cannot cut off the rest of the file.
- Right-clicking a document tab offers "Refresh File" and, for Markdown, "Export PDF", in addition to the usual close actions.

### 1.4 New documents and PDF export

- "New document" opens an empty draft that is not yet on disk and does not appear in the project tree. The first save asks where to store it: the desktop app uses the system Save As dialog, and browser clients and remote connection windows show a Save As dialog for the machine that runs the session. Syntax highlighting follows the chosen file name. Right-clicking a draft's tab also offers "Save to Disk…".
- "Export PDF" turns a Markdown document into a vector PDF with automatic page breaks and embedded CJK fonts. The desktop app asks where to save the file; browser clients download it.

## 2. Browser tabs (desktop app only)

Browser tabs show real web pages inside VelaTerm, which is useful for keeping documentation, issue pages or a local preview next to your sessions. They are available only in the desktop app, not in browser clients, remote connection windows or the mobile layout.

**Opening a browser tab:**

- the "Built-in Browser" button at the right end of the tab bar;
- the ⌘⇧B shortcut;
- `vopen <URL>` in a session;
- "New Browser Page" in a project's or group's context menu in the sidebar.

**Toolbar:** "Back", "Forward", "Reload" (or "Stop loading" while a page loads), an address bar ("Enter URL or search terms"; text that is not an address is searched on Google), "Open in system browser", and a "Quick access" row with ChatGPT, Claude, Gemini and Google.

**Two forms:**

- **Browser tab**: opened with the button, ⌘⇧B or `vopen <URL>`. It is not saved in the project tree and does not survive a restart.
- **Browser page node**: created with "New Browser Page". It is a node in the project tree that remembers its address and is still there after a restart.

**Links that open new windows:** when a page opens a link in a new window (for example a `target="_blank"` link), VelaTerm opens it in a new browser tab.

**Standalone pages:** the "Game Center" button at the right end of the tab bar opens the VelaTerm game center as a standalone page. A standalone page has no address bar or quick access row, only "Back", "Forward" and "Reload"; links it opens stay in the same tab.

**Security boundary:** an embedded page is treated as an ordinary external website. It receives no VelaTerm permissions or credentials, and only `http` and `https` addresses can be opened.

## 3. How the tab kinds coexist

- Session, document and browser tabs share one tab bar, and ⌘1–9 switches among all of them.
- Opening a session from the tree reuses only session tabs; document and browser tabs are never replaced.
- Document and browser tabs cannot be split. ⌘W closes them directly (a document with unsaved changes asks first).
