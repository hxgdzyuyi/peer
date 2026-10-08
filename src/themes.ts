import githubLight from "@shikijs/themes/github-light";
import githubDark from "@shikijs/themes/github-dark";
import oneDark from "@shikijs/themes/one-dark-pro";
import dracula from "@shikijs/themes/dracula";
import latte from "@shikijs/themes/catppuccin-latte";
import mocha from "@shikijs/themes/catppuccin-mocha";
import { registerCustomTheme, type ThemeRegistration } from "@pierre/diffs";
import type { ImportedTheme } from "./types";

export type Theme = ImportedTheme & { builtin?: boolean };
export const builtins: Theme[] = [
  ["github-light", "GitHub Light", "light", githubLight],
  ["github-dark", "GitHub Dark", "dark", githubDark],
  ["one-dark-pro", "One Dark Pro", "dark", oneDark],
  ["dracula", "Dracula", "dark", dracula],
  ["catppuccin-latte", "Catppuccin Latte", "light", latte],
  ["catppuccin-mocha", "Catppuccin Mocha", "dark", mocha],
].map(([id, name, kind, definition]) => ({
  id: id as string,
  name: name as string,
  kind: kind as "light" | "dark",
  definition: definition as ThemeRegistration,
  builtin: true,
}));
const registered = new Set<string>();
export function registerThemes(themes: Theme[]) {
  for (const theme of themes) {
    // Namespaced built-ins avoid conflicts with Pierre's own lazy theme catalog.
    const name = rendererThemeId(theme);
    if (!registered.has(name)) {
      registerCustomTheme(name, async () => ({ ...theme.definition, name }));
      registered.add(name);
    }
  }
}
export const rendererThemeId = (theme: Theme) =>
  theme.builtin ? `inspector-${theme.id}` : theme.id;

export function themeVariables(theme: Theme): Record<string, string> {
  const dark = theme.kind === "dark";
  const defaults: Record<string, string> = {
    bg: dark ? "#0d1117" : "#ffffff",
    surface: dark ? "#161b22" : "#f6f8fa",
    raised: dark ? "#21262d" : "#ffffff",
    fg: dark ? "#e6edf3" : "#24292f",
    muted: dark ? "#8b949e" : "#6e7781",
    border: dark ? "#30363d" : "#d8dee4",
    accent: dark ? "#58a6ff" : "#0969da",
    accentFg: "#ffffff",
    selection: dark ? "#1f3a5f" : "#ddf4ff",
    add: dark ? "#3fb950" : "#1a7f37",
    remove: dark ? "#f85149" : "#cf222e",
    addBg: dark ? "#12261e" : "#e6ffec",
    removeBg: dark ? "#321b20" : "#ffebe9",
    addWord: dark ? "#2ea04355" : "#abf2bc",
    removeWord: dark ? "#f8514955" : "#ffcecb",
    input: dark ? "#0d1117" : "#ffffff",
    button: dark ? "#238636" : "#1f883d",
    focus: dark ? "#58a6ff" : "#0969da",
  };
  const c = theme.definition.colors ?? {};
  const take = (key: string, ...keys: string[]) => {
    for (const color of keys) {
      if (c[color]) {
        defaults[key] = c[color];
        break;
      }
    }
  };
  take("bg", "editor.background");
  take("fg", "editor.foreground", "foreground");
  take(
    "surface",
    "sideBar.background",
    "panel.background",
    "editor.background",
  );
  take(
    "raised",
    "editorWidget.background",
    "panel.background",
    "editor.background",
  );
  take("muted", "descriptionForeground", "editorLineNumber.foreground");
  take("border", "panel.border", "sideBar.border", "editorGroup.border");
  take("accent", "textLink.foreground", "focusBorder");
  take(
    "selection",
    "list.activeSelectionBackground",
    "editor.selectionBackground",
  );
  take("input", "input.background");
  take("button", "button.background");
  take("accentFg", "button.foreground");
  take("focus", "focusBorder");
  take(
    "add",
    "gitDecoration.addedResourceForeground",
    "editorGutter.addedBackground",
  );
  take(
    "remove",
    "gitDecoration.deletedResourceForeground",
    "editorGutter.deletedBackground",
  );
  take("addBg", "diffEditor.insertedLineBackground");
  take("removeBg", "diffEditor.removedLineBackground");
  take("addWord", "diffEditor.insertedTextBackground");
  take("removeWord", "diffEditor.removedTextBackground");
  return Object.fromEntries(
    Object.entries(defaults).map(([key, value]) => [
      `--${key.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)}`,
      value,
    ]),
  );
}
export function applyTheme(theme: Theme) {
  const element = document.documentElement;
  for (const [key, value] of Object.entries(themeVariables(theme)))
    element.style.setProperty(key, value);
  element.style.colorScheme = theme.kind;
  element.dataset.theme = theme.kind;
}
