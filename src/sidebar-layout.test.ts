import { describe, expect, it } from "vitest";
import { fitSidebars, minimumSourceWidth } from "./sidebar-layout";

describe("sidebar layout", () => {
  it("fits wide saved panels into a narrow window without overwriting the preference", () => {
    const preferred = { files: 600, feedback: 640 };
    const fitted = fitSidebars(preferred, 950, true);
    expect(fitted.files).toBeGreaterThanOrEqual(180);
    expect(fitted.feedback).toBeGreaterThanOrEqual(260);
    expect(950 - fitted.files - fitted.feedback).toBeGreaterThanOrEqual(
      minimumSourceWidth,
    );
    expect(fitSidebars(preferred, 1800, true)).toEqual(preferred);
  });
  it("reserves no space for hidden feedback and preserves its saved width", () => {
    expect(fitSidebars({ files: 600, feedback: 640 }, 950, false)).toEqual({
      files: 590,
      feedback: 640,
    });
  });
  it("bounds saved widths and keeps the source visible throughout supported window sizes", () => {
    for (let width = 950; width <= 1800; width += 17) {
      for (const preferred of [
        { files: 244, feedback: 310 },
        { files: 180, feedback: 640 },
        { files: 600, feedback: 260 },
        { files: 1000, feedback: 1000 },
      ]) {
        const fitted = fitSidebars(preferred, width, true);
        expect(fitted.files).toBeGreaterThanOrEqual(180);
        expect(fitted.files).toBeLessThanOrEqual(600);
        expect(fitted.feedback).toBeGreaterThanOrEqual(260);
        expect(fitted.feedback).toBeLessThanOrEqual(640);
        expect(width - fitted.files - fitted.feedback).toBeGreaterThanOrEqual(
          minimumSourceWidth,
        );
      }
    }
  });
});
