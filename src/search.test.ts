import { expect, it } from "vitest";
import { textMatches } from "./search";

it("treats search punctuation literally and finds every occurrence", () => {
  expect(textMatches("asdf(); asdf(); asdf(1)", "ASDF();")).toEqual([
    { start: 0, end: 7 },
    { start: 8, end: 15 },
  ]);
  expect(textMatches("[a+b].*", "[a+b].*")).toEqual([{ start: 0, end: 7 }]);
});

it("retains original UTF-16 offsets after Unicode characters", () => {
  expect(textMatches("İ 😀 asdf", "ASDF")).toEqual([{ start: 5, end: 9 }]);
  expect(textMatches("İ 😀 x İ 😀 x", "😀 x")).toEqual([
    { start: 2, end: 6 },
    { start: 9, end: 13 },
  ]);
});

it("handles empty, missing and adjacent matches", () => {
  expect(textMatches("text", "")).toEqual([]);
  expect(textMatches("text", "missing")).toEqual([]);
  expect(textMatches("aaaa", "aa")).toEqual([
    { start: 0, end: 2 },
    { start: 2, end: 4 },
  ]);
});
