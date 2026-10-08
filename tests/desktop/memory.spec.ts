import { browser, $, expect } from "@wdio/globals";
import { execFileSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { cpus, platform, release } from "node:os";

// Use physical footprint, which includes compressed memory on macOS. RSS alone
// hides most of a WebKit process's memory after it has been compressed.
function processes() {
  const binary = resolve(process.env.APP_BINARY ?? "target/debug/peer");
  const listing = execFileSync("ps", ["-axo", "pid=,comm="], {
    encoding: "utf8",
  });
  const match = listing
    .split("\n")
    .map((line) => line.trim().match(/^(\d+)\s+(.+)$/))
    .filter((line) => line?.[2] === binary)
    .sort((a, b) => Number(b![1]) - Number(a![1]))[0];
  if (!match) throw new Error("Disposable test application not found");
  const main = Number(match[1]);
  const domain = execFileSync("launchctl", ["print", `pid/${main}`], {
    encoding: "utf8",
  });
  const webContent = Number(
    domain.match(
      /^\s*([1-9]\d*)[ \t]+[^\n]*com\.apple\.WebKit\.WebContent/m,
    )?.[1],
  );
  if (!webContent) {
    writeFileSync(
      "test-results/memory-process-domain.txt",
      domain.match(/services = \{[^}]*\}/)?.[0] ?? "No process services",
    );
    throw new Error(
      `Test app's WebContent process not found (main PID ${main})`,
    );
  }
  return { main, webContent };
}

function footprint(pid: number) {
  const output = execFileSync(
    "footprint",
    ["-p", String(pid), "--noCategories"],
    { encoding: "utf8" },
  );
  const number = (name: string) => {
    const match = output.match(
      new RegExp(`${name}:\\s*([\\d.]+)\\s*(KB|MB|GB|B)`),
    );
    if (!match) throw new Error(`Missing ${name} for ${pid}`);
    return (
      Number(match[1]) *
      { B: 1, KB: 1024, MB: 1024 ** 2, GB: 1024 ** 3 }[match[2]]!
    );
  };
  return {
    bytes: number("phys_footprint"),
    peakBytes: number("phys_footprint_peak"),
  };
}

describe("Native memory workload", () => {
  it("records memory over repeated navigation and refresh", async () => {
    await $('[data-repository="benchmark-repo"]').waitForExist();
    await browser.execute(async () => {
      await (window as any).__TAURI__.window.getCurrentWindow().setFocus();
    });
    let pids: ReturnType<typeof processes>;
    await browser.waitUntil(
      () => {
        try {
          pids = processes();
          return true;
        } catch {
          return false;
        }
      },
      {
        timeoutMsg:
          "Could not identify the test WebContent process; see memory-process-domain.txt",
      },
    );
    const samples: unknown[] = [];
    const sample = async (phase: string) => {
      // Allow normal GC/worker completion; never force GC or hide compressed pages.
      await browser.pause(1500);
      const ui = await browser.execute(() => ({
        loadedFiles: Number(
          document.querySelector(".app")?.getAttribute("data-loaded-files"),
        ),
        highlightBytes: document.documentElement.dataset.highlightCacheBytes,
        highlightEntries:
          document.documentElement.dataset.highlightCacheEntries,
        errors: [
          document.querySelector(".fatal")?.textContent,
          ...Array.from(document.querySelectorAll("diffs-container")).map(
            (e) =>
              e.shadowRoot?.querySelector("[data-error-wrapper]")?.textContent,
          ),
        ].filter(Boolean),
      }));
      expect(ui.errors).toEqual([]);
      if (ui.highlightBytes != null) {
        expect(Number(ui.highlightBytes)).toBeLessThanOrEqual(24 * 1024 * 1024);
        expect(Number(ui.highlightEntries)).toBeLessThanOrEqual(12);
        if (phase === "code-100-files")
          expect(ui.loadedFiles).toBeLessThanOrEqual(8);
      }
      samples.push({
        phase,
        ...ui,
        main: footprint(pids.main),
        webContent: footprint(pids.webContent),
      });
      writeFileSync(
        process.env.PEER_MEMORY_REPORT ?? "docs/memory-macos.json",
        JSON.stringify(
          {
            machine: {
              cpu: cpus()[0].model,
              platform: platform(),
              release: release(),
            },
            fixture: { files: 100, linesPerFile: 1000, changedLines: 10000 },
            build: "Tauri debug with embedded test driver",
            samples,
          },
          null,
          2,
        ) + "\n",
      );
    };
    const visit = async (i: number) => {
      const path = `src/module-${String(i).padStart(3, "0")}.ts`;
      await browser.execute(
        (path) =>
          document
            .querySelector<HTMLElement>(`.tree-file[title="${path}"]`)!
            .click(),
        path,
      );
      await browser.waitUntil(async () =>
        browser.execute(
          (path, i) => {
            const host = document
              .querySelector(`[data-file-path="${path}"]`)
              ?.closest("diffs-container");
            const ready =
              !!host?.shadowRoot?.querySelector("[data-code] span") &&
              host.shadowRoot
                .querySelector("[data-code]")!
                .textContent?.includes(`item${i}_0`);
            // This is a memory workload, not a latency measurement. A first
            // jump can precede the asynchronous layout of adjacent files.
            if (!ready)
              document
                .querySelector<HTMLElement>(`.tree-file[title="${path}"]`)
                ?.click();
            return ready;
          },
          path,
          i,
        ),
      );
      await browser.pause(150);
    };
    await visit(0);
    await sample("startup");
    for (let pass = 1; pass <= 2; pass++) {
      for (let i = 0; i < 100; i++) {
        await visit(i);
        if ([19, 49, 99].includes(i))
          await sample(`changes-pass-${pass}-file-${i + 1}`);
      }
    }
    await $('[aria-label="Refresh repository"]').click();
    await visit(0);
    await sample("changes-refresh");
    await $("button=Code").click();
    await $(".breadcrumb").waitForExist();
    for (let i = 0; i < 100; i++) await visit(i);
    await sample("code-100-files");
    await $("button=Changes").click();
    await $(".breadcrumb").waitForExist({ reverse: true });
    await visit(0);
    await sample("return-to-changes");
  });
});
