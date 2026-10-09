import { describe, expect, it } from "vitest";
import {
  builtins,
  themeVariables,
  rendererThemeId,
  type Theme,
} from "./themes";

describe("offline theme compatibility", () => {
  it("bundles six distinct, complete syntax themes", () => {
    expect(builtins).toHaveLength(6);
    expect(new Set(builtins.map(rendererThemeId)).size).toBe(6);
    for (const theme of builtins) {
      expect(theme.definition.tokenColors?.length).toBeGreaterThan(0);
      expect(themeVariables(theme)["--bg"]).toMatch(/^#/);
    }
  });
  it("maps workbench colors and supplies mode-specific missing colors", () => {
    const theme: Theme = {
      id: "test",
      name: "test",
      kind: "dark",
      definition: {
        colors: {
          "editor.background": "#123456",
          "button.background": "#abcdef",
          focusBorder: "#ff0000",
        },
      },
    };
    expect(themeVariables(theme)).toMatchObject({
      "--bg": "#123456",
      "--button": "#abcdef",
      "--focus": "#ff0000",
      "--fg": "#e6edf3",
    });
    expect(themeVariables({ ...theme, kind: "light" })["--fg"]).toBe("#24292f");
  });
});
