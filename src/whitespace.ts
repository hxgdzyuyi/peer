// Decorations contain only the original characters; the markers are CSS paint.
// Re-run only on newly rendered text nodes, so virtualization bounds the work.
export function decorateWhitespace(root: ShadowRoot | HTMLElement) {
  for (const line of root.querySelectorAll<HTMLElement>(
    "[data-line][data-line-type]",
  )) {
    const walker = document.createTreeWalker(line, NodeFilter.SHOW_TEXT);
    const nodes: Text[] = [];
    while (walker.nextNode()) {
      const text = walker.currentNode as Text;
      if (
        /[ \t]/.test(text.data) &&
        !text.parentElement?.closest("[data-inspector-whitespace]")
      )
        nodes.push(text);
    }
    for (const text of nodes) {
      const fragment = document.createDocumentFragment();
      for (const part of text.data.split(/([ \t]+)/)) {
        if (!part) continue;
        if (/^[ \t]+$/.test(part)) {
          for (const run of part.match(/ +|\t/g) ?? []) {
            const span = document.createElement("span");
            span.dataset.inspectorWhitespace = run === "\t" ? "tab" : "space";
            span.textContent = run;
            fragment.append(span);
          }
        } else fragment.append(document.createTextNode(part));
      }
      text.replaceWith(fragment);
    }
  }
}
export const whitespaceCSS = `
  [data-inspector-whitespace="space"] { background-image: radial-gradient(circle at .5ch 55%, var(--muted) .65px, transparent .9px); background-size: 1ch 100%; }
  [data-inspector-whitespace="tab"] { position: relative; }
  [data-inspector-whitespace="tab"]::after { content: '→'; position: absolute; left: 0; top: 0; color: var(--muted); pointer-events: none; }
`;
