---
name: vartifact
description: >-
  Show the user a visual result as a live page in VelaTerm. Use when the user asks to see, mock up, sketch or
  prototype a screen, page, form, chart or diagram, rather than to read code: write one self-contained HTML file
  and open it with `vopen`, which renders it beside this conversation and reloads it on every save. Available only
  inside VelaTerm-hosted local sessions.
argument-hint: "[what to show]"
allowed-tools: Bash(vopen:*)
---

# vartifact

VelaTerm previews HTML pages as artifacts. A page opened with `vopen` from this conversation appears in a panel
beside it, reloads whenever the file changes, and keeps each saved version so the user can step back through them.
Errors the page logs show in the panel's console, and the user can send them back to you.

User input:

$ARGUMENTS

## Step 1: Decide where the page goes

- Use the path the user named. Otherwise put it next to the code it is about, or in the working directory, with a
  short descriptive name such as `clone-dialog.html`.
- Keep editing the same file across rounds. A new file starts a new history; the same file builds one.

## Step 2: Write the page

- One self-contained HTML file: inline `<style>` and `<script>`. Files beside it (`./style.css`, `./app.js`,
  images) also load, if the page really needs them.
- The preview starts **offline**: requests to the internet, including CDNs and web fonts, are blocked unless the
  user allows the network for that file. Use system fonts and inline everything the page needs.
- Use realistic sample content, not lorem ipsum, and make the layout work at narrow widths: the panel is often
  half a screen.

### Showing several options

When the user asks for alternatives, or the direction is open, put two to four options side by side on one page
and mark each with `data-vlx-option`:

```html
<section data-vlx-option="1a" data-vlx-option-label="Filled primary button">…</section>
<section data-vlx-option="1b" data-vlx-option-label="Action tiles">…</section>
```

- IDs are short and stable: a round number plus a letter (`1a`, `1b`, then `2a` for the next round). Never reuse
  or renumber an ID; add a new round above the older ones instead.
- The label is a few words saying how the option differs. Without one, the option's first heading is used.
- VelaTerm lists the marked options above the preview. The user can jump to each and pick one, which sends you a
  message such as "go with option 1b". Then keep that design, apply it, and remove the other options.

## Step 3: Open it

```bash
vopen <absolute path>
```

Run it once; later edits reload the preview on their own, so there is no need to run it again.

## Step 4: Report back

Say in one or two lines what the page shows and what to look at. Do not paste the HTML into the conversation; the
user is looking at the page.

## Comments from the preview

The user can click elements in the preview and comment on them. The comments arrive as a message listing each
element by tag, visible text, CSS selector and option, followed by the note. Change those elements in place, keep
the rest of the page as it is, and save the same file.

## Notes

- `vopen` needs a VelaTerm-hosted local session. If it reports that it is not inside one, tell the user the
  preview is only available in VelaTerm and give the file path instead.
- If the user reports errors from the preview, fix the page in place.
