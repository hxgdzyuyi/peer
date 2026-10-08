import { browser, $, expect } from "@wdio/globals";
import { writeFileSync, appendFileSync, readFileSync } from "node:fs";
import { resolve, join } from "node:path";
import { cpus, platform, release } from "node:os";

async function click(selector: string) {
  for (let attempt = 0; ; attempt++) {
    try {
      await $(selector).waitForEnabled();
      await $(selector).click();
      return;
    } catch (error) {
      if (attempt >= 2 || !String(error).includes("no longer attached"))
        throw error;
    }
  }
}

describe("Peer in the native webview", () => {
  beforeEach(async () => {
    await browser.execute(async () => {
      await (window as any).__TAURI__.window.getCurrentWindow().setFocus();
    });
  });
  afterEach(async () => {
    const errors = await browser.execute(() =>
      [
        document.querySelector(".fatal")?.textContent,
        ...Array.from(document.querySelectorAll("diffs-container")).map(
          (host) =>
            host.shadowRoot?.querySelector("[data-error-wrapper]")?.textContent,
        ),
      ].filter(Boolean),
    );
    expect(errors).toEqual([]);
  });
  it("opens a real 100-file Git comparison and highlights offline", async () => {
    await $('[data-repository="benchmark-repo"]').waitForExist();
    await browser.waitUntil(
      async () => Number(await $(".app").getAttribute("data-loaded-files")) > 0,
    );
    await expect($(".statusbar")).toHaveText(
      expect.stringContaining("100 files"),
    );
    writeFileSync(
      "test-results/dom.json",
      JSON.stringify(
        await browser.execute(() =>
          Array.from(document.querySelectorAll("diffs-container"))
            .slice(0, 2)
            .map((e) => ({
              html: e.shadowRoot?.innerHTML.slice(-10000),
              style: e.getAttribute("style"),
            })),
        ),
        null,
        2,
      ),
    );
    await browser.waitUntil(async () =>
      browser.execute(() =>
        Array.from(document.querySelectorAll("diffs-container")).some(
          (e) => !!e.shadowRoot?.querySelector("[data-code] span"),
        ),
      ),
    );
    await browser.saveScreenshot(resolve("test-results/changes.png"));
  });

  it("saves feedback, keeps a draft through theme changes, and exports real anchors", async () => {
    await click('[aria-label="Comment on file src/module-000.ts"]');
    await $('[aria-label="Comment text"]').setValue(
      "Please explain the value change.",
    );
    await click("button=Add comment");
    await $(".comment-card").waitForExist();
    await click('[aria-label="Comment on file src/module-000.ts"]');
    await $('[aria-label="Comment text"]').setValue(
      "Draft survives theme switching",
    );
    await click('[aria-label="Settings"]');
    await click("button*=Dracula");
    await click('[aria-label="Close dialog"]');
    await browser.waitUntil(async () =>
      browser.execute(() =>
        Array.from(document.querySelectorAll("diffs-container")).some(
          (e) =>
            e.shadowRoot?.querySelector("pre") &&
            getComputedStyle(e.shadowRoot.querySelector("pre")!)
              .backgroundColor === "rgb(40, 42, 54)",
        ),
      ),
    );
    await expect($('[aria-label="Comment text"]')).toHaveValue(
      "Draft survives theme switching",
    );
    await click('[aria-label="Discard draft"]');
    await click("button=Copy JSON");
    await expect($(".toast")).toHaveText(expect.stringContaining("copied"));
    await browser.saveScreenshot(resolve("test-results/dark-feedback.png"));
  });

  it("preserves whitespace text while showing markers and switches layouts", async () => {
    const before = await browser.execute(() =>
      Array.from(document.querySelectorAll("diffs-container")).map(
        (e) =>
          e.shadowRoot?.querySelector("[data-line][data-line-type]")
            ?.textContent,
      ),
    );
    await click('[aria-label="Toggle whitespace markers"]');
    const after = await browser.execute(() =>
      Array.from(document.querySelectorAll("diffs-container")).map(
        (e) =>
          e.shadowRoot?.querySelector("[data-line][data-line-type]")
            ?.textContent,
      ),
    );
    expect(after).toEqual(before);
    expect(before.some((text) => text?.includes("\t"))).toBe(true);
    expect(
      await browser.execute(() =>
        Array.from(document.querySelectorAll("diffs-container")).some(
          (e) => !!e.shadowRoot?.querySelector("[data-inspector-whitespace]"),
        ),
      ),
    ).toBe(true);
    await click('[aria-label="Toggle diff layout"]');
    await click('[aria-label="Toggle diff layout"]');
  });

  it("reaches clipped line endings with a visible scrollbar in a narrow window", async () => {
    const resize = (width: number, height: number) =>
      browser.execute(
        async (w, h) => {
          const api = (window as any).__TAURI__.window;
          await api.getCurrentWindow().setSize(new api.LogicalSize(w, h));
        },
        width,
        height,
      );
    const readColumns = () =>
      browser.execute(() => {
        const path = document
          .querySelector('.tree-file[aria-current="true"]')!
          .getAttribute("title");
        const host = Array.from(document.querySelectorAll("[data-file-path]"))
          .find((e) => e.getAttribute("data-file-path") === path)!
          .closest("diffs-container")!;
        return Array.from(
          host.shadowRoot!.querySelectorAll<HTMLElement>("[data-code]"),
        ).map((code) => {
          const line = code.querySelector("[data-line][data-line-type]")!;
          const range = document.createRange();
          range.selectNodeContents(line);
          return {
            text: line.textContent,
            left: code.scrollLeft,
            end: range.getBoundingClientRect().right,
            edge: code.getBoundingClientRect().left + code.clientWidth,
          };
        });
      });
    try {
      await resize(950, 740);
      await click('.tree-file[title="src/module-000.ts"]');
      for (const mode of ["unified", "split", "code"]) {
        if (mode === "split") await click('[aria-label="Toggle diff layout"]');
        if (mode === "code") {
          await click("button=Code");
          await $(".breadcrumb").waitForExist();
          await click('.tree-file[title="src/module-000.ts"]');
        }
        await browser.waitUntil(async () =>
          browser.execute((mode) => {
            const root = document
              .querySelector('[data-file-path="src/module-000.ts"]')
              ?.closest("diffs-container")?.shadowRoot;
            return (
              !!root?.querySelector(
                mode === "code"
                  ? "[data-file]"
                  : mode === "split"
                    ? '[data-diff-type="split"]'
                    : '[data-diff-type="single"]',
              ) &&
              !!root
                ?.querySelector("[data-code]")
                ?.textContent?.includes("item0_0")
            );
          }, mode),
        );
        const bar = $(".viewer-horizontal-scrollbar");
        await bar.waitForDisplayed();
        const geometry = await browser.execute(() => {
          const bar = document.querySelector(".viewer-horizontal-scrollbar")!;
          const rect = bar.getBoundingClientRect();
          const viewport = document
            .querySelector(".code-view")!
            .getBoundingClientRect();
          return {
            top: rect.top,
            bottom: rect.bottom,
            viewportBottom: viewport.bottom,
            height: innerHeight,
            range: bar.scrollWidth - bar.clientWidth,
          };
        });
        expect(geometry.range).toBeGreaterThan(20);
        expect(geometry.top).toBeGreaterThanOrEqual(
          geometry.viewportBottom - 1,
        );
        expect(geometry.bottom).toBeLessThan(geometry.height);
        await browser.execute(() =>
          document
            .querySelector<HTMLElement>(".viewer-horizontal-scrollbar")!
            .focus(),
        );
        // The embedded driver sends raw WebDriver private-use characters for
        // Home/End. Dispatch standard DOM key events to exercise the real handler.
        const press = (key: string) =>
          browser.execute((key) => {
            document.activeElement!.dispatchEvent(
              new KeyboardEvent("keydown", {
                key,
                bubbles: true,
                cancelable: true,
              }),
            );
          }, key);
        await press("Home");
        await browser.waitUntil(async () =>
          (await readColumns()).every((c) => c.left === 0),
        );
        const before = await readColumns();
        expect(before.some((c) => c.end > c.edge)).toBe(true);
        await press("End");
        try {
          await browser.waitUntil(async () =>
            (await readColumns()).every(
              (c) => c.left > 0 && c.end <= c.edge + 1,
            ),
          );
        } catch (error) {
          writeFileSync(
            "test-results/horizontal-failure.json",
            JSON.stringify(
              {
                mode,
                columns: await readColumns(),
                bar: await browser.execute(() => {
                  const bar = document.querySelector(
                    ".viewer-horizontal-scrollbar",
                  )!;
                  return {
                    left: bar.scrollLeft,
                    width: bar.clientWidth,
                    full: bar.scrollWidth,
                    focused: document.activeElement?.outerHTML,
                    inner: bar.outerHTML,
                  };
                }),
              },
              null,
              2,
            ),
          );
          await browser.saveScreenshot(
            resolve("test-results/horizontal-failure.png"),
          );
          throw error;
        }
        const after = await readColumns();
        expect(after.map((c) => c.text)).toEqual(before.map((c) => c.text));
        expect(after).toHaveLength(mode === "split" ? 2 : 1);
        await browser.saveScreenshot(
          resolve(`test-results/horizontal-${mode}.png`),
        );
        // Scrolling directly over the code must also update the bottom bar.
        await browser.execute(() => {
          const host = document
            .querySelector('[data-file-path="src/module-000.ts"]')!
            .closest("diffs-container")!;
          const columns = Array.from(
            host.shadowRoot!.querySelectorAll<HTMLElement>("[data-code]"),
          );
          columns.sort(
            (a, b) =>
              b.scrollWidth - b.clientWidth - (a.scrollWidth - a.clientWidth),
          )[0].scrollLeft = 20;
        });
        await browser.waitUntil(
          async () =>
            Math.abs(
              (await browser.execute(
                () =>
                  document.querySelector(".viewer-horizontal-scrollbar")!
                    .scrollLeft,
              )) - 20,
            ) < 2,
        );
      }
      await click("button=Changes");
      await click('[aria-label="Toggle diff layout"]');
      await click('.tree-file[title="src/module-001.ts"]');
      await expect($(".viewer-horizontal-scrollbar")).toHaveAttribute(
        "aria-label",
        "Scroll code horizontally: src/module-001.ts",
      );
      await $(".viewer-horizontal-scrollbar").waitForDisplayed();
      await click('[aria-label="Collapse src/module-001.ts"]');
      await browser.waitUntil(
        async () => !(await $(".viewer-horizontal-scrollbar").isDisplayed()),
      );
      await click('[aria-label="Expand src/module-001.ts"]');
      await $(".viewer-horizontal-scrollbar").waitForDisplayed();
    } finally {
      await resize(1440, 940);
      await click('.tree-file[title="src/module-000.ts"]');
    }
  });

  it("resizes code and diffs immediately, preserving drafts and saved preferences", async () => {
    const readState = () =>
      JSON.parse(
        readFileSync(
          join(process.env.PEER_DATA_DIR!, "state.json"),
          "utf8",
        ),
      );
    const queuesBefore = readState().queues;
    const chooseFont = (size: number) =>
      browser.execute((value) => {
        // The embedded driver's option clicks don't emit a select change event.
        const select =
          document.querySelector<HTMLSelectElement>("#code-font-size")!;
        select.value = String(value);
        select.dispatchEvent(new Event("change", { bubbles: true }));
      }, size);
    const waitForFont = async (size: number, height: number) => {
      await browser.waitUntil(async () =>
        browser.execute(
          (font, lineHeight) => {
            const lines = Array.from(
              document.querySelectorAll("diffs-container"),
            ).flatMap((node) =>
              Array.from(
                node.shadowRoot?.querySelectorAll(
                  "[data-line][data-line-type]",
                ) ?? [],
              ),
            );
            return (
              lines.length > 0 &&
              lines.every((line) => {
                const style = getComputedStyle(line);
                return (
                  style.fontSize === `${font}px` &&
                  style.lineHeight === `${lineHeight}px`
                );
              })
            );
          },
          size,
          height,
        ),
      );
    };
    await click('[aria-label="Comment on file src/module-000.ts"]');
    await $('[aria-label="Comment text"]').setValue(
      "Keep this draft while resizing",
    );
    await click('[aria-label="Settings"]');
    await chooseFont(16);
    await click('[aria-label="Close dialog"]');
    await waitForFont(16, 29);
    await expect($('[aria-label="Comment text"]')).toHaveValue(
      "Keep this draft while resizing",
    );
    await click('[aria-label="Discard draft"]');
    await browser.waitUntil(
      async () => readState().preferences.codeFontSize === 16,
    );
    expect(readState().queues).toEqual(queuesBefore);
    await click("button=Code");
    await waitForFont(16, 29);
    await browser.refresh();
    await $('[data-repository="benchmark-repo"]').waitForExist();
    await waitForFont(16, 29);
    await click("button=Changes");
    await waitForFont(16, 29);
    await click('[aria-label="Settings"]');
    await expect($("#code-font-size")).toHaveValue("16");
    await chooseFont(12);
    await click('[aria-label="Close dialog"]');
    await waitForFont(12, 22);
  });

  it("jumps beyond the viewport and saves a one-based line anchor and Reviewed state", async () => {
    await click('button[aria-label="Find in file"]');
    await $('input[aria-label="Go to line"]').setValue("80");
    await browser.keys("Enter");
    await $('[aria-label="Comment on selected line"]').waitForExist();
    await click('[aria-label="Comment on selected line"]');
    await $('[aria-label="Comment text"]').setValue("Can we name this value?");
    await $("button=Add comment").waitForEnabled();
    // Exercise native form submission; the embedded driver's synthetic click
    // does not reliably perform a submit button's default action after reload.
    const inputState = await browser.execute(() => {
      const form =
        document.querySelector<HTMLFormElement>(".comment-composer")!;
      const button = form.querySelector<HTMLButtonElement>(".primary-button")!;
      const state = {
        disabled: button.disabled,
        text: form.querySelector("textarea")!.value,
      };
      form.requestSubmit(button);
      return state;
    });
    writeFileSync("test-results/line-submit.json", JSON.stringify(inputState));
    expect(inputState.disabled).toBe(false);
    await browser.waitUntil(async () =>
      (await $(".feedback-title").getText()).includes("2"),
    );
    await click('.tree-file[title="src/module-000.ts"]');
    await click('[data-file-path="src/module-000.ts"] .review-check input');
    await browser.waitUntil(async () =>
      (await $(".statusbar").getText()).includes("1 reviewed"),
    );
    const saved = JSON.parse(
      readFileSync(
        join(process.env.PEER_DATA_DIR!, "state.json"),
        "utf8",
      ),
    );
    const queue: any = Object.values(saved.queues).find(
      (q: any) => q.comments.length === 2,
    );
    expect(queue.comments[1].anchor.line).toBe(80);
    expect(queue.comments[1].anchor.side).toBe("new");
    expect(queue.comments[1].anchor.lineText).toContain("item0_79");
    expect(queue.comments[0].anchor.line).toBeUndefined();
    expect(Object.keys(queue.reviewed)).toEqual(["src/module-000.ts"]);
    await click('[aria-label="Close search"]');
  });

  it("marks saved feedback outdated after refresh and keeps separate Code feedback", async () => {
    appendFileSync(
      join(process.env.PEER_TEST_REPO!, "src/module-000.ts"),
      "\n// changed after review\n",
    );
    await click('[aria-label="Refresh repository"]');
    await $(".outdated-tag").waitForExist();
    await click("button=Code");
    await $(".breadcrumb").waitForExist();
    await expect($(".feedback-panel")).toHaveText(
      expect.stringContaining("General code review"),
    );
    await expect($(".comment-card")).not.toExist();
    await click("button=Changes");
    await $(".outdated-tag").waitForExist();
  });

  it("follows diff scrolling in the file navigator without moving focus or jumping back", async () => {
    await click('.tree-file[title="src/module-040.ts"]');
    await $('[data-file-path="src/module-040.ts"]').waitForExist();
    await expect($('.tree-file[aria-current="true"]')).toHaveAttribute(
      "title",
      "src/module-040.ts",
    );
    await browser.waitUntil(async () =>
      browser.execute(
        () =>
          document
            .querySelector('[data-file-path="src/module-040.ts"]')
            ?.closest("diffs-container")
            ?.shadowRoot?.querySelector("[data-code]")
            ?.textContent?.includes("item40_") === true,
      ),
    );
    // Re-selecting the current file must not leave a jump queued for later.
    await click('.tree-file[title="src/module-040.ts"]');
    await click(".tree-folder");
    await expect($(".tree-folder")).toHaveAttribute("aria-expanded", "false");
    await click('[aria-label="Filter files"]');

    const scrollToFile = async (path: string, direction: number) => {
      // Scroll the actual diff container, without clicking a navigation item.
      // Wait for adjacent source to load, then scroll until its sticky header
      // reaches the viewport. A pinned header is not the file's logical start.
      await browser.waitUntil(
        async () =>
          browser.execute(
            (path, direction) => {
              const scroll = document.querySelector<HTMLElement>(".code-view")!;
              const header = Array.from(
                document.querySelectorAll<HTMLElement>("[data-file-path]"),
              ).find((node) => node.dataset.filePath === path);
              if (header) {
                const host = header.closest("diffs-container");
                const index = Number(path.match(/module-(\d+)\.ts$/)?.[1]);
                if (
                  !host?.shadowRoot
                    ?.querySelector("[data-code]")
                    ?.textContent?.includes(`item${index}_`)
                )
                  return false;
                const bounds = header.getBoundingClientRect();
                const top = scroll.getBoundingClientRect().top;
                if (bounds.top <= top + 1 && bounds.bottom > top + 1)
                  return true;
              }
              scroll.scrollTop += direction * 500;
              return false;
            },
            path,
            direction,
          ),
        { timeout: 10000, interval: 100 },
      );
      await expect($('.tree-file[aria-current="true"]')).toHaveAttribute(
        "title",
        path,
      );
      await browser.waitUntil(async () =>
        browser.execute(() => {
          const tree = document
            .querySelector(".file-tree")!
            .getBoundingClientRect();
          const active = document
            .querySelector('.tree-file[aria-current="true"]')!
            .getBoundingClientRect();
          return active.top >= tree.top - 1 && active.bottom <= tree.bottom + 1;
        }),
      );
      expect(
        await browser.execute(() =>
          document.activeElement?.getAttribute("aria-label"),
        ),
      ).toBe("Filter files");
    };

    await scrollToFile("src/module-041.ts", 1);
    await expect($(".tree-folder")).toHaveAttribute("aria-expanded", "true");
    await browser.waitUntil(async () => {
      const state = JSON.parse(
        readFileSync(
          join(process.env.PEER_DATA_DIR!, "state.json"),
          "utf8",
        ),
      );
      return (
        state.preferences.sessions[state.preferences.lastRepository]
          ?.selectedPath === "src/module-041.ts"
      );
    });
    await scrollToFile("src/module-040.ts", -1);
    // Sidebar clicks still navigate, and the passive selection does not undo it.
    await click('.tree-file[title="src/module-010.ts"]');
    await expect($('.tree-file[aria-current="true"]')).toHaveAttribute(
      "title",
      "src/module-010.ts",
    );
    await $('[data-file-path="src/module-010.ts"]').waitForExist();
  });

  it("measures warm navigation and scrolling in WKWebView", async function () {
    // Keep focused benchmark runs comparable with the full desktop suite.
    await click('[aria-label="Settings"]');
    await click("button*=Dracula");
    await click('[aria-label="Close dialog"]');
    if (
      !(
        await $('[aria-label="Toggle whitespace markers"]').getAttribute(
          "class",
        )
      )
        ?.split(" ")
        .includes("active")
    )
      await click('[aria-label="Toggle whitespace markers"]');
    await click('.tree-file[title="src/module-010.ts"]');
    await $('[data-file-path="src/module-010.ts"]').waitForExist();
    await browser.execute(() => {
      const state = {
        frames: [] as number[],
        navigation: [] as number[],
        done: false,
        started: performance.now(),
      };
      Object.assign(window, { __benchmark: state });
      const scroll = document.querySelector(".code-view") as HTMLElement;
      let previous = performance.now(),
        frame = 0,
        nav = 0;
      function navigate() {
        const start = performance.now();
        (
          document.querySelector(
            `.tree-file[title="src/module-${nav % 2 ? "010" : "011"}.ts"]`,
          ) as HTMLElement
        )?.click();
        requestAnimationFrame(() =>
          requestAnimationFrame(() => {
            state.navigation.push(performance.now() - start);
            // With only 18 retained samples, p95 is the maximum. Keep enough
            // samples to measure the percentile separately from isolated stalls.
            if (++nav < 100) navigate();
            else state.done = true;
          }),
        );
      }
      function tick(time: number) {
        state.frames.push(time - previous);
        previous = time;
        scroll.scrollTop += frame < 60 ? 35 : -35;
        if (++frame < 120) requestAnimationFrame(tick);
        else navigate();
      }
      requestAnimationFrame(tick);
    });
    try {
      await browser.waitUntil(
        () =>
          browser.execute(() => (window as any).__benchmark.frames.length > 2),
        { timeout: 5000, interval: 100 },
      );
    } catch {
      writeFileSync(
        "test-results/performance.json",
        JSON.stringify(
          {
            status: "unmeasured",
            reason:
              "WKWebView animation frames are suspended by the host; rerun in an active desktop session.",
            reference: {
              cpu: cpus()[0].model,
              platform: platform(),
              release: release(),
            },
          },
          null,
          2,
        ),
      );
      this.skip();
      return;
    }
    await browser.waitUntil(
      () => browser.execute(() => (window as any).__benchmark.done),
      { timeout: 30000 },
    );
    const metrics = await browser.execute(() => {
      const state = (window as any).__benchmark;
      const frames = state.frames
        .slice(2)
        .sort((a: number, b: number) => a - b);
      const nav = state.navigation
        .slice(2)
        .sort((a: number, b: number) => a - b);
      return {
        scrollP95Ms: frames[Math.floor(frames.length * 0.95)],
        warmNavigationP95Ms: nav[Math.floor(nav.length * 0.95)],
        frames,
        navigation: nav,
      };
    });
    writeFileSync(
      "test-results/performance.json",
      JSON.stringify(
        {
          ...metrics,
          reference: {
            cpu: cpus()[0].model,
            platform: platform(),
            release: release(),
          },
          workload: { files: 100, changedLines: 10000 },
        },
        null,
        2,
      ),
    );
    expect(metrics.scrollP95Ms).toBeLessThan(33);
    expect(metrics.warmNavigationP95Ms).toBeLessThan(100);
  });
  it("keeps plain-text layouts consistent through collapse, feedback, and theme updates", async () => {
    await click("button=Code");
    await $(".breadcrumb").waitForExist();
    await click('.tree-file[title="notes.txt"]');
    const readViewer = () =>
      browser.execute(() => {
        const root = document
          .querySelector('[data-file-path="notes.txt"]')
          ?.closest("diffs-container")?.shadowRoot;
        return {
          error: root?.querySelector("[data-error-wrapper]")?.textContent ?? "",
          text: root?.querySelector("[data-code]")?.textContent ?? "",
        };
      });
    await browser.waitUntil(async () =>
      (await readViewer()).text.includes("Plain text line 1"),
    );
    await click('[aria-label="Collapse notes.txt"]');
    expect((await readViewer()).error).toBe("");
    await click('[aria-label="Expand notes.txt"]');
    await browser.waitUntil(async () =>
      (await readViewer()).text.includes("Plain text line 1"),
    );
    expect((await readViewer()).error).toBe("");
    await click('[aria-label="Comment on file notes.txt"]');
    await $('[aria-label="Comment text"]').setValue(
      "Please clarify this plain-text document.",
    );
    await browser.execute(() => {
      const form =
        document.querySelector<HTMLFormElement>(".comment-composer")!;
      form.requestSubmit(
        form.querySelector<HTMLButtonElement>(".primary-button")!,
      );
    });
    await $(".comment-card").waitForExist();
    expect((await readViewer()).error).toBe("");
    for (const theme of ["Dracula", "GitHub Light"]) {
      await click('[aria-label="Settings"]');
      await click(`button*=${theme}`);
      await click('[aria-label="Close dialog"]');
      await browser.waitUntil(async () =>
        (await readViewer()).text.includes("Plain text line 1"),
      );
      expect((await readViewer()).error).toBe("");
    }
    writeFileSync(
      join(process.env.PEER_TEST_REPO!, "notes.txt"),
      "Updated plain text after refresh\n",
    );
    await click('[aria-label="Refresh repository"]');
    await browser.waitUntil(async () =>
      (await readViewer()).text.includes("Updated plain text after refresh"),
    );
    expect((await readViewer()).error).toBe("");
    await $(".outdated-tag").waitForExist();
    await click("button=Changes");
  });

  // Native dragging changes OS window state; run it after scroll measurements.

  it("reloads evicted Code files without losing feedback or an open draft", async () => {
    await click("button=Code");
    await $(".breadcrumb").waitForExist();
    await click('.tree-file[title="src/module-000.ts"]');
    await click('[aria-label="Comment on file src/module-000.ts"]');
    await $('[aria-label="Comment text"]').setValue(
      "Draft survives source eviction",
    );
    for (let i = 1; i <= 20; i++) {
      const path = `src/module-${String(i).padStart(3, "0")}.ts`;
      await click(`.tree-file[title="${path}"]`);
      await browser.waitUntil(async () =>
        browser.execute(
          (path, i) => {
            const host = document
              .querySelector(`[data-file-path="${path}"]`)
              ?.closest("diffs-container");
            return host?.shadowRoot
              ?.querySelector("[data-code]")
              ?.textContent?.includes(`item${i}_0`);
          },
          path,
          i,
        ),
      );
    }
    expect(
      Number(await $(".app").getAttribute("data-loaded-files")),
    ).toBeLessThanOrEqual(8);
    await expect($('[aria-label="Comment text"]')).toHaveValue(
      "Draft survives source eviction",
    );
    await browser.execute(() => {
      const button = Array.from(
        document.querySelectorAll<HTMLButtonElement>("button"),
      ).find((b) => b.textContent === "Add comment")!;
      button.form!.requestSubmit(button);
    });
    await browser.waitUntil(async () =>
      (await $(".feedback-panel").getText()).includes(
        "Draft survives source eviction",
      ),
    );
    await click('.tree-file[title="src/module-000.ts"]');
    await browser.waitUntil(async () =>
      browser.execute(() => {
        const host = document
          .querySelector('[data-file-path="src/module-000.ts"]')
          ?.closest("diffs-container");
        return (
          host?.shadowRoot
            ?.querySelector("[data-code]")
            ?.textContent?.includes("item0_0") &&
          host.textContent?.includes("Draft survives source eviction")
        );
      }),
    );
  });

  it("accepts native dragging from the title bar and app name, excluding buttons", async () => {
    // Exercise Tauri's real drag handler and IPC permissions. Synthetic WebDriver
    // events cannot verify physical movement by the OS window manager.
    const dragHandled = await browser.execute(async () => {
      const handled: boolean[] = [];
      for (const selector of [
        ".titlebar",
        ".app-wordmark span",
        '[aria-label="Settings"]',
      ]) {
        const box = document.querySelector(selector)!.getBoundingClientRect();
        const x =
          selector === ".titlebar" ? box.left + 180 : box.left + box.width / 2;
        const y = box.top + box.height / 2;
        const target = document.elementFromPoint(x, y)!;
        for (const type of ["mousedown", "mouseup"]) {
          const event = new MouseEvent(type, {
            bubbles: true,
            composed: true,
            cancelable: true,
            button: 0,
            detail: 1,
            clientX: x,
            clientY: y,
          });
          target.dispatchEvent(event);
          if (type === "mousedown") handled.push(event.defaultPrevented);
        }
      }
      // Exercise the actual command, so a missing capability rejects the test.
      await (window as any).__TAURI__.window.getCurrentWindow().startDragging();
      return handled;
    });
    expect(dragHandled).toEqual([true, true, false]);
    await click('[aria-label="Settings"]');
    await $(".settings-modal").waitForExist();
    await click('[aria-label="Close dialog"]');
  });
});
