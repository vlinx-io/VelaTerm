# Local Knowledge Bases

Created: 2026-09-10

Updated: 2026-09-25 10:21

A local knowledge base is an ordinary folder of Markdown files that you write and organize in VelaTerm. Notes and attachments stay in that folder, so you can keep editing them with other editors. Some menu items call a local knowledge base a "notebook"; both words refer to the same folder.

The built-in "Session Knowledge Base" sits next to your local knowledge bases. It holds knowledge that agents organize from your sessions and keeps its own entries, sources and revision history; see [Session Knowledge Base](global-memory_20260905_2027.md).

## Opening the knowledge base

Click the knowledge icon in the right panel (tooltip "Knowledge Base"). A "Knowledge Base" tab opens in the center with the home page:

- a search box ("Search session knowledge and local notes…", see [Searching](#searching));
- the "Session Knowledge Base";
- your "Local knowledge bases";
- the "Open knowledge base" and "Create knowledge base" buttons.

The right panel shows a tree with three roots: "Session Knowledge Base", "Archived Sessions" and "Local knowledge bases". Click an arrow to expand a folder in place; several knowledge bases can be expanded at once. Click a folder name to show its contents, and click a note to open it. The "Knowledge bases" link at the top of the tree returns to the home page, and the "Up one level" button in a page header goes to the parent page without leaving the knowledge base.

Notes open in ordinary document tabs, with the same editing, saving, search and PDF export as any other Markdown file (see [Document & Browser Tabs](document-and-browser-tabs_20260709_2041.md)). The "Knowledge Base" tab stays in the tab bar so that you can return to where you were.

## Opening or creating a local knowledge base

- **Open:** click "Open knowledge base" and choose a folder. The desktop app shows the system folder picker; a browser client shows a folder browser for the connected computer. An existing Obsidian vault can be opened directly: files stay where they are, and hidden folders such as `.obsidian` are neither indexed nor changed.
- **Create:** click "Create knowledge base", enter a "Name", choose a "Folder" (or "Browse"; the suggestion is `~/Notes`), and confirm. VelaTerm creates the folder and opens it. The parent folder must exist, and a folder with the same name must not; otherwise VelaTerm shows "The destination already exists. Choose another name or folder." To use an existing folder, choose "Open knowledge base" instead.
- **Rename:** right-click a knowledge base in the tree and choose "Rename". This renames the folder on disk.
- **Close:** click "Close notebook" in the knowledge base toolbar, or right-click it in the tree. Closing removes the folder from the list; its files remain on disk. When you open the same folder again, its trash and import history are still there.

"Local" means the computer that runs the VelaTerm backend you are connected to. In a browser client, "Open knowledge base" browses that computer's folders, and importing uploads the files you choose in the browser to that computer.

## Importing files and folders

1. Open the target knowledge base and click "Import" in its toolbar.
2. Choose the destination "Folder", then "Choose files", "Choose folder", or drag files and folders into the import area.
3. Check the list and confirm.

The import runs in the background, so you can switch pages or open notes while it copies. Its progress appears in the knowledge base toolbar and on the home page, and it can be canceled from the import dialog or from "Import history". When it finishes, a banner shows the number of imported and skipped files; if the window is not in focus, a system notification ("Import finished" or "Import failed") appears. Only one import can run in a knowledge base at a time.

An import copies files as they are, including non-ASCII file names, relative folder structure and attachments, and never modifies the originals. Importing a folder includes the folder itself. If any file would overwrite an existing one, the whole batch is rejected. Hidden files and hidden configuration folders are skipped. Browser file pickers do not provide empty folders, so empty folders are not imported.

One import can contain up to 5,000 files (hidden files that are skipped count toward this limit) and up to 1 GiB in total, with at most 100 MiB per file. Files are staged first and appear in the destination only after the whole batch has arrived, so an interrupted upload never shows up as a half-written note.

"Import history" lists every import with its time, destination ("Knowledge base root" for the top folder), status ("Importing", "Completed", "Failed", "Cancelled" or "Interrupted"), the number of imported and skipped files, the size and the duration. Click "Files (N)" to see each file and why it was skipped. "Delete record" removes the entry from the history; the imported files are not removed. An import whose progress has stopped for more than two minutes, for example because the app quit unexpectedly, is marked "Interrupted"; deleting its record also removes the files that were staged for it.

## Writing and organizing notes

A knowledge base page has the views "Recent notes", "Trash" and "Import history", a "New note" button, and toolbar buttons for "New folder", "Import" and "Close notebook". The note list can be filtered by tag ("All tags") and sorted by "Recently updated" or "Title".

- While a knowledge base page is shown, ⌘N / Ctrl+N creates a "New note", ⌘O / Ctrl+O opens "Quick open" (find a note by its path), and ⌘⇧F / Ctrl+Shift+F moves to the "Search notes…" box.
- To rename a note, right-click it in the tree and choose "Rename". To move it, drag it onto another folder of the same knowledge base. "Delete" moves it to the knowledge base's trash.
- When a folder is open, "Rename or move" moves the folder to a path relative to the knowledge base root; the parent folder must already exist. Links in other notes of the same knowledge base are updated when a note or folder moves, keeping the link text and leaving code untouched.
- Deleted items go to a `.vkb-trash` folder inside the knowledge base. Open "Trash" and click "Restore" to bring an item back; an existing file with the same name is never overwritten.
- Tags come from `#tag` in the text and from a YAML `tags` list or array.
- Images pasted into a note are saved in an `assets` folder next to the note.

The tree and lists refresh about every five seconds while the page is visible, and when the window regains focus. An open document tab detects changes made by other programs and offers to reload.

Knowledge base pages, dialogs, the import history, searches, search scope, tags, sorting and quick open all have their own URLs, so you can refresh the page or use the browser's back and forward buttons. Unsaved text and form input are not part of the URL.

## Searching

- **Home page:** searches the Session Knowledge Base and all local knowledge bases at once and groups the results by source. Archived sessions are not included.
- **Right panel:** "Search notes…" above the tree searches all local knowledge bases.
- **Knowledge base page:** "Search notes…" searches note names, paths and text. "Search scope" switches between "This knowledge base" and "All knowledge bases".

Several words must all match. Each result shows its knowledge base or session group, a snippet, the line number, the number of matches and the "Related notes" the note links to; click any of them to open it. Use Up and Down to select a result, Enter to open it and Esc to clear the search. Results are ordered by matches in the name or path, then by the number of matches, then by the most recent update. Up to 100 results are listed; when there are more, the list ends with "Only the first results are listed. Narrow the search to see the rest."

VelaTerm looks for exact matches first and falls back to approximate matches only when there are none, showing "No exact matches. Showing approximate results." Approximate matching applies to words made of Latin letters and digits: letters in order (`knwl` finds `knowledge`), initials (`kb` finds `knowledge base`), and small typing errors (one error in words of four or more characters, two in words of eight or more; `knoledge` finds `knowledge`). Chinese and other non-Latin text is matched exactly.

## Links and compatibility with existing files

VelaTerm recognizes the common `[[note]]`, `[[folder/note|label]]` and `[[note#heading]]` forms, standard relative Markdown links, and `![[image.png]]`. It uses them to update links when notes move, to list "Related notes" in search results, and for the `vkb` commands below. In the editor, `[[…]]` links are shown as plain text. When several files share a name, a short link is resolved only if the target is unique; otherwise write the folder path.

Files remain plain Markdown, and YAML front matter stays in the file; Source mode in the editor shows it exactly. Markdown files that are not UTF-8 or larger than 5 MiB are not listed as notes and are counted under "Skipped". A knowledge base can contain up to 50,000 files and folders; a larger folder cannot be opened and shows "The file or selection exceeds the notebook limits." Symbolic links are not followed.

These features cover everyday work with local files, folders, Markdown, links and search. Obsidian plugins, themes, Canvas, block references, embedded notes and YAML aliases are not supported; the corresponding files are left unchanged. For the file model and link syntax, see Obsidian's documentation on [vaults](https://help.obsidian.md/vault), [internal links](https://help.obsidian.md/links) and [attachments](https://help.obsidian.md/attachments).

## Copying from the Session Knowledge Base and agent queries

Open an entry in the Session Knowledge Base, choose "Copy to a local notebook", then pick the target knowledge base and file name. The copy is an independent Markdown note; the original entry, its source snapshots and its revision history stay where they are.

In a VelaTerm session, you or an agent can read registered local knowledge bases with `vkb`:

```sh
vkb notes "reading notes"
vkb note <knowledge-base-ID> "reading/how-to-take-notes.md"
vkb search "architecture decision"
```

- `notes` searches every local knowledge base registered on the current backend. Each result includes `vaultId`, `vaultName`, `path`, `absolutePath`, `line`, `matches`, `score`, `matched` (the text that actually matched), `related` (notes this note links to) and `summary` (the text around the match). The response also reports `total`, `hasMore`, `unavailable` (knowledge bases that could not be read) and `fuzzy` (whether the results are approximate). Up to 100 results are returned.
- `note` returns a note's text, outline, links, backlinks and tags.
- `search` returns matches from the Session Knowledge Base, local notes and, when a code index exists, the code graph (see [Code Graph & Knowledge Base](codegraph_20260905_2027.md)).

These commands are read-only. They never change notes, and VelaTerm does not add knowledge base content to sessions automatically.

## Storage and logs

The files on disk are the authoritative copy of your notes and attachments. The application database (`kb_*` tables) stores only the list of registered knowledge bases, trash records, import history and a search cache that can be rebuilt. The Session Knowledge Base uses its own `memory_*` tables.

Opening, writing, moving, deleting, restoring and importing are recorded in the runtime log with an operation ID, counts and the time taken, never with note contents. These lines use the `knowledge` event; when `VLX_LOG_DIR` is not set, `VLX_KNOWLEDGE_LOG_DIR` sets their directory. Import history is stored in the database and shown on the "Import history" page. See [Runtime logs and privacy](runtime-diagnostics_20260909.md) for log locations and privacy. To back up a knowledge base completely, keep both its folder and the application data, which holds its trash and import records.
