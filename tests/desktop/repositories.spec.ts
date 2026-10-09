import { browser, $, $$, expect } from "@wdio/globals";
import { mkdirSync, mkdtempSync, realpathSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { execFileSync } from "node:child_process";
import { defaultComparison } from "../../src/types";

const workspace = realpathSync(
  mkdtempSync(join(tmpdir(), "peer-repositories-")),
);
const first = join(workspace, "first", "shared");
const second = join(workspace, "second", "shared");
const missing = join(workspace, "missing");
const trigger = '[aria-label="Switch repository"]';
const menu = '[role="menu"][aria-label="Repositories"]';
let originalPreferences: unknown;

async function waitForRepository(path: string) {
  await browser.waitUntil(
    async () =>
      (await $(trigger).getAttribute("title")) === path &&
      (await $(trigger).isEnabled()),
  );
}
async function switchTo(path: string) {
  await $(trigger).click();
  await $(`[data-repository-path="${path}"]`).click();
  await waitForRepository(path);
}
async function menuKey(key: string) {
  await browser.execute((key) => {
    document.activeElement?.dispatchEvent(
      new KeyboardEvent("keydown", {
        key,
        bubbles: true,
        cancelable: true,
      }),
    );
  }, key);
}
describe("Repository switcher", () => {
  before(async () => {
    for (const root of [first, second]) {
      mkdirSync(root, { recursive: true });
      const git = (...args: string[]) =>
        execFileSync("git", args, { cwd: root, stdio: "pipe" });
      git("init", "-b", "main");
      git("config", "user.name", "Test");
      git("config", "user.email", "test@example.invalid");
      for (const name of ["first", "second"])
        writeFileSync(
          join(root, `${name}.ts`),
          Array.from(
            { length: 160 },
            (_, i) => `export const ${name}${i} = ${i};\n`,
          ).join(""),
        );
      git("add", ".");
      git("commit", "-m", "base");
      writeFileSync(join(root, "first.ts"), "export const changed = true;\n");
    }
    originalPreferences = await browser.execute(
      async () =>
        (await (window as any).__TAURI__.core.invoke("bootstrap")).preferences,
    );
    await browser.execute(
      async ({ first, second, missing, comparison }) => {
        const api = (window as any).__TAURI__.core;
        const { preferences } = await api.invoke("bootstrap");
        await api.invoke("save_preferences", {
          preferences: {
            ...preferences,
            lastRepository: first,
            recentRepositories: [first, second, missing],
            sessions: {
              [first]: { comparison, selectedPath: "first.ts", scrollTop: 0 },
            },
          },
        });
        setTimeout(() => location.reload(), 100);
      },
      {
        first,
        second,
        missing,
        comparison: { ...defaultComparison, view: "code" },
      },
    );
    await waitForRepository(first);
  });
  beforeEach(async () => {
    await browser.execute(async () => {
      await (window as any).__TAURI__.window.getCurrentWindow().setFocus();
    });
  });
  after(async () => {
    await browser.execute(async (preferences) => {
      await (window as any).__TAURI__.core.invoke("save_preferences", {
        preferences,
      });
    }, originalPreferences);
  });

  it("distinguishes matching names and supports keyboard navigation and dismissal", async () => {
    await $(trigger).click();
    await expect($(`${menu} [aria-checked="true"]`)).toHaveAttribute(
      "data-repository-path",
      first,
    );
    expect(await $$(`${menu} strong`).map((item) => item.getText())).toEqual([
      "shared",
      "shared",
      "missing",
    ]);
    await expect($(`[data-repository-path="${second}"] small`)).toHaveText(
      second,
    );
    await expect($(`[data-repository-path="${first}"]`)).toBeFocused();
    await menuKey("ArrowDown");
    await expect($(`[data-repository-path="${second}"]`)).toBeFocused();
    await menuKey("End");
    await expect($(`[data-repository-path="${missing}"]`)).toBeFocused();
    await menuKey("Home");
    await expect($(`[data-repository-path="${first}"]`)).toBeFocused();
    await browser.saveScreenshot(
      resolve("test-results/repository-switcher.png"),
    );
    await menuKey("Escape");
    await expect($(menu)).not.toExist();
    await expect($(trigger)).toBeFocused();
    await $(trigger).click();
    await browser.keys("Tab");
    await expect($(menu)).not.toExist();
    await $(trigger).click();
    await $(".statusbar").click();
    await expect($(menu)).not.toExist();
  });

  it("restores each repository's view, selected file, scroll and draft", async () => {
    await $('.tree-file[title="second.ts"]').click();
    await $('[data-file-path="second.ts"]').waitForExist();
    await $('[aria-label="Comment on file second.ts"]').click();
    await $('[aria-label="Comment text"]').setValue(
      "Draft retained across repositories",
    );
    const top = await browser.execute(() => {
      const view = document.querySelector<HTMLElement>(".code-view")!;
      view.scrollTop = 900;
      view.dispatchEvent(new Event("scroll"));
      return view.scrollTop;
    });
    expect(top).toBeGreaterThan(0);
    await switchTo(second);
    await expect($(".view-switch button.active")).toHaveText("Changes");
    await expect($(".comment-composer")).not.toExist();
    await switchTo(first);
    await expect($(".view-switch button.active")).toHaveText("Code");
    await expect($('.tree-file[title="second.ts"]')).toHaveAttribute(
      "aria-current",
      "true",
    );
    await browser.waitUntil(
      async () =>
        Math.abs(
          (await browser.execute(
            () => document.querySelector(".code-view")!.scrollTop,
          )) - top,
        ) < 3,
    );
    await expect($('[aria-label="Comment text"]')).toHaveValue(
      "Draft retained across repositories",
    );
    await $("button=Add comment").click();
    await expect($(".comment-card")).toHaveText(
      expect.stringContaining("Draft retained across repositories"),
    );
    await expect($(".comment-composer")).not.toExist();
  });

  it("keeps the current repository when a saved path fails", async () => {
    await $(trigger).click();
    await $(`[data-repository-path="${missing}"]`).click();
    await $(".error-banner").waitForDisplayed();
    await waitForRepository(first);
    await $('[aria-label="Dismiss error"]').click();
    await expect($(".repository-add")).toBeEnabled();
  });

  it("remembers the chosen repository after reload", async () => {
    await switchTo(second);
    await $(trigger).click();
    await expect($(`${menu} [aria-checked="true"]`)).toHaveAttribute(
      "data-repository-path",
      second,
    );
    await expect($(`[data-repository-path="${first}"]`)).toExist();
    await menuKey("Escape");
    await browser.waitUntil(async () =>
      browser.execute(async (path) => {
        const { preferences } = await (window as any).__TAURI__.core.invoke(
          "bootstrap",
        );
        return (
          preferences.lastRepository === path &&
          preferences.recentRepositories.includes(path)
        );
      }, second),
    );
    await browser.execute(() => location.reload());
    await waitForRepository(second);
    await switchTo(first);
    await expect($(".view-switch button.active")).toHaveText("Code");
    await expect($('.tree-file[title="second.ts"]')).toHaveAttribute(
      "aria-current",
      "true",
    );
  });
});
