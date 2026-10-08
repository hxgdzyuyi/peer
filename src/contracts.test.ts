import { expect, it } from "vitest";
import preferences from "../tests/contracts/preferences.json";
import { defaultPreferences } from "./types";
it("matches the shared Rust serialization contract for preferences", () => {
  expect(defaultPreferences).toEqual(preferences);
});
