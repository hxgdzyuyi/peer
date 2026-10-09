import { Fragment } from "react";

export type TextMatch = { start: number; end: number };
export type SourceMatch = TextMatch & { line: number; side: "old" | "new" };

/** Literal, case-insensitive matches with offsets in the original UTF-16 text. */
export function textMatches(text: string, query: string): TextMatch[] {
  if (!query) return [];
  const pattern = new RegExp(
    query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"),
    "giu",
  );
  return Array.from(text.matchAll(pattern), (match) => ({
    start: match.index,
    end: match.index + match[0].length,
  }));
}

export function SearchText({ text, query }: { text: string; query: string }) {
  const matches = textMatches(text, query);
  if (!matches.length) return text;
  return (
    <>
      {matches.map((match, index) => (
        <Fragment key={match.start}>
          {text.slice(index ? matches[index - 1].end : 0, match.start)}
          <mark className="search-match">
            {text.slice(match.start, match.end)}
          </mark>
        </Fragment>
      ))}
      {text.slice(matches[matches.length - 1].end)}
    </>
  );
}

const decorated = new WeakMap<
  HTMLElement,
  {
    query: string;
    text: string;
    marks: HTMLElement[];
  }
>();

/** Paint across syntax/whitespace spans without changing source text or token offsets. */
export function highlightSearchLine(
  line: HTMLElement,
  query: string,
  active?: TextMatch,
) {
  const previous = decorated.get(line);
  if (!query && !previous) return;
  const text = line.textContent ?? "";
  let marks = previous?.marks ?? [];
  if (
    !previous ||
    previous.query !== query ||
    previous.text !== text ||
    marks.some((mark) => !line.contains(mark))
  ) {
    for (const mark of line.querySelectorAll("mark[data-search-start]")) {
      const parent = mark.parentNode;
      mark.replaceWith(...mark.childNodes);
      parent?.normalize();
    }
    marks = [];
    const matches = textMatches(text, query);
    if (matches.length) {
      const walker = document.createTreeWalker(line, NodeFilter.SHOW_TEXT);
      const nodes: Text[] = [];
      while (walker.nextNode()) nodes.push(walker.currentNode as Text);
      let offset = 0;
      let matchIndex = 0;
      for (const node of nodes) {
        const end = offset + node.length;
        while (matchIndex < matches.length && matches[matchIndex].end <= offset)
          matchIndex++;
        const fragment = document.createDocumentFragment();
        let cursor = 0;
        for (
          let i = matchIndex;
          i < matches.length && matches[i].start < end;
          i++
        ) {
          const match = matches[i];
          const start = Math.max(match.start - offset, 0);
          const stop = Math.min(match.end - offset, node.length);
          fragment.append(node.data.slice(cursor, start));
          const mark = document.createElement("mark");
          mark.className = "search-match";
          mark.dataset.searchStart = String(match.start);
          mark.dataset.searchEnd = String(match.end);
          mark.textContent = node.data.slice(start, stop);
          fragment.append(mark);
          marks.push(mark);
          cursor = stop;
        }
        if (cursor) {
          fragment.append(node.data.slice(cursor));
          node.replaceWith(fragment);
        }
        offset = end;
      }
    }
    if (query) decorated.set(line, { query, text, marks });
    else decorated.delete(line);
  }
  for (const mark of marks) {
    mark.classList.toggle(
      "search-match-active",
      !!active &&
        Number(mark.dataset.searchStart) === active.start &&
        Number(mark.dataset.searchEnd) === active.end,
    );
  }
}

export const searchCSS = `
  mark.search-match { background: #ffe58a; color: #24292f; border-radius: 2px; }
  mark.search-match-active { background: #ffad42; color: #24292f; outline: 1px solid #9a5100; }
`;
