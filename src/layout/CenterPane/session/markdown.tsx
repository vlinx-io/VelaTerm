//! Markdown rendering for session-view message bubbles.
//!
//! Agents answer in Markdown, so a bubble that showed the raw text would be full of `**` and stray fences.
//! `marked` tokenizes; this module maps the tokens straight to React nodes. Going through tokens rather than
//! `marked.parse` keeps HTML out of the picture entirely — nothing is ever fed to `dangerouslySetInnerHTML`,
//! so a transcript that quotes markup cannot inject anything into the app.
//!
//! Only what agent answers actually contain is handled. Raw HTML blocks render as their own source text,
//! which is both honest and safe.

import { marked, type Token, type Tokens } from "marked";
import type { ReactNode } from "react";

import { highlight } from "../../RightPanel/highlight";
import { SessionLink } from "./links";
import {
  codeBlockCopyAttributes,
  copyAttributes,
  listItemCopyAttributes,
  orderedListCopyAttributes,
  tableCellCopyAttributes,
} from "./selectionCopy";

/** Beyond this length a bubble renders as plain text: parsing megabytes of tool chatter is not worth a frame drop. */
const PARSE_LIMIT = 200_000;

/** Render a Markdown string as React nodes. */
export function Markdown({ text }: { text: string }): ReactNode {
  if (text.length > PARSE_LIMIT) {
    return (
      <div className="sv-p" {...copyAttributes.p}>
        {text}
      </div>
    );
  }
  let tokens: Token[];
  try {
    tokens = marked.lexer(text);
  } catch {
    // A tokenizer failure must not blank out the message; show the source instead.
    return (
      <div className="sv-p" {...copyAttributes.p}>
        {text}
      </div>
    );
  }
  return <>{blocks(tokens)}</>;
}

/** Render a list of block-level tokens. */
function blocks(tokens: Token[]): ReactNode[] {
  return tokens.map((tk, i) => <Block key={i} token={tk} />);
}

function Block({ token }: { token: Token }): ReactNode {
  switch (token.type) {
    case "space":
      return null;
    case "heading": {
      const h = token as Tokens.Heading;
      return (
        <div
          className="sv-h"
          style={{ fontSize: h.depth <= 2 ? "1.12em" : "1.04em" }}
          data-sv-markdown-tag={`h${h.depth}`}
        >
          {inline(h.tokens)}
        </div>
      );
    }
    case "paragraph":
      return (
        <div className="sv-p" {...copyAttributes.p}>
          {inline((token as Tokens.Paragraph).tokens)}
        </div>
      );
    case "text": {
      const t = token as Tokens.Text;
      return (
        <div className="sv-p" {...copyAttributes.p}>
          {t.tokens ? inline(t.tokens) : t.text}
        </div>
      );
    }
    case "code": {
      const c = token as Tokens.Code;
      return <CodeBlock code={c.text} lang={c.lang} />;
    }
    case "blockquote":
      return (
        <div className="sv-quote" {...copyAttributes.blockquote}>
          {blocks((token as Tokens.Blockquote).tokens)}
        </div>
      );
    case "list": {
      const l = token as Tokens.List;
      return (
        <div
          className="sv-list"
          {...(l.ordered ? orderedListCopyAttributes(l.start) : copyAttributes.ul)}
        >
          {l.items.map((item, i) => (
            <div
              className="sv-li"
              key={i}
              {...listItemCopyAttributes(item.task, item.checked)}
            >
              <span className="sv-li-mark" {...copyAttributes.listMarker}>
                {l.ordered ? `${Number(l.start || 1) + i}.` : item.task ? (item.checked ? "☑" : "☐") : "•"}
              </span>
              <span className="sv-li-body">{blocks(item.tokens)}</span>
            </div>
          ))}
        </div>
      );
    }
    case "table": {
      const tb = token as Tokens.Table;
      return (
        <div className="sv-table-wrap">
          <table className="sv-table" {...copyAttributes.table}>
            <thead {...copyAttributes.thead}>
              <tr {...copyAttributes.tr}>
                {tb.header.map((cell, i) => (
                  <th key={i} {...tableCellCopyAttributes("th", tb.align[i])}>
                    {inline(cell.tokens)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody {...copyAttributes.tbody}>
              {tb.rows.map((row, r) => (
                <tr key={r} {...copyAttributes.tr}>
                  {row.map((cell, c) => (
                    <td key={c} {...tableCellCopyAttributes("td", tb.align[c])}>
                      {inline(cell.tokens)}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      );
    }
    case "hr":
      return <div className="sv-hr" {...copyAttributes.hr} />;
    default:
      // html, def, and anything marked adds later: show the source text rather than dropping content.
      return (
        <div className="sv-p" {...copyAttributes.p}>
          {(token as Tokens.Generic).raw ?? ""}
        </div>
      );
  }
}

/** A fenced code block with lightweight highlighting and its language shown when declared. */
function CodeBlock({ code, lang }: { code: string; lang?: string }): ReactNode {
  return (
    <div className="sv-code" {...codeBlockCopyAttributes(lang)}>
      {lang ? (
        <div className="sv-code-lang" {...copyAttributes.ignore}>
          {lang}
        </div>
      ) : null}
      <pre {...copyAttributes.code}>{highlight(code)}</pre>
    </div>
  );
}

/** Render inline tokens: emphasis, code spans, links, and line breaks. */
function inline(tokens: Token[] | undefined): ReactNode[] {
  if (!tokens) return [];
  return tokens.map((tk, i) => {
    switch (tk.type) {
      case "text": {
        const t = tk as Tokens.Text;
        return t.tokens ? <span key={i}>{inline(t.tokens)}</span> : <span key={i}>{t.text}</span>;
      }
      case "codespan":
        return (
          <code className="sv-codespan" key={i} {...copyAttributes.code}>
            {(tk as Tokens.Codespan).text}
          </code>
        );
      case "strong":
        return (
          <strong key={i} {...copyAttributes.strong}>
            {inline((tk as Tokens.Strong).tokens)}
          </strong>
        );
      case "em":
        return (
          <em key={i} {...copyAttributes.em}>
            {inline((tk as Tokens.Em).tokens)}
          </em>
        );
      case "del":
        return (
          <del key={i} {...copyAttributes.s}>
            {inline((tk as Tokens.Del).tokens)}
          </del>
        );
      case "br":
        return <br key={i} {...copyAttributes.br} />;
      case "link": {
        const l = tk as Tokens.Link;
        return (
          <SessionLink href={l.href} key={i}>
            {inline(l.tokens)}
          </SessionLink>
        );
      }
      case "image": {
        const im = tk as Tokens.Image;
        // Transcript image paths point at local files the WebView cannot load; show the description instead.
        return (
          <span className="sv-img" key={i}>
            {im.text || im.href}
          </span>
        );
      }
      case "escape":
        return <span key={i}>{(tk as Tokens.Escape).text}</span>;
      default:
        return <span key={i}>{(tk as Tokens.Generic).raw ?? ""}</span>;
    }
  });
}
