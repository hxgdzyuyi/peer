import { browser, $ } from "@wdio/globals";
import type { Preferences } from "../../src/types";

export async function reloadApp(preferences: Preferences | null = null) {
  const loadedAt = await browser.execute((preferences) => {
    // The test build consumes this once, before mounting the replacement app.
    // Saving here would race the outgoing document's debounced autosave.
    if (preferences)
      sessionStorage.setItem(
        "peer:desktop-preferences",
        JSON.stringify(preferences),
      );
    return performance.timeOrigin;
  }, preferences);
  await browser.refresh();
  await browser.waitUntil(() =>
    browser.execute(
      (before) =>
        performance.timeOrigin !== before && !!document.querySelector(".app"),
      loadedAt,
    ),
  );
  await $('[aria-label="Switch repository"]').waitForEnabled();
}
