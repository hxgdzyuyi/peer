import { describe, expect, it } from "vitest";
import {
  assessPerformance,
  enforcePerformance,
  performanceMode,
} from "./performance";

describe("desktop performance policy", () => {
  it("enforces the existing targets by default", () => {
    const report = assessPerformance(
      { scrollP95Ms: 41, warmNavigationP95Ms: 67 },
      performanceMode(),
    );
    expect(report.status).toBe("over-budget");
    expect(() => enforcePerformance(report)).toThrow("scrollP95Ms: 41 ms");
  });

  it("reports CI overruns without treating them as within budget", () => {
    const report = assessPerformance(
      { scrollP95Ms: 41, warmNavigationP95Ms: 101 },
      performanceMode("report"),
    );
    expect(report.status).toBe("over-budget");
    expect(report.violations).toHaveLength(2);
    expect(() => enforcePerformance(report)).not.toThrow();
  });

  it("keeps strict boundaries and recognizes measurements within budget", () => {
    expect(
      assessPerformance({ scrollP95Ms: 21, warmNavigationP95Ms: 67 }, "enforce")
        .status,
    ).toBe("within-budget");
    expect(
      assessPerformance(
        { scrollP95Ms: 33, warmNavigationP95Ms: 100 },
        "enforce",
      ).violations,
    ).toHaveLength(2);
  });

  it("rejects invalid policies and invalid measurements even in report mode", () => {
    expect(() => performanceMode("off")).toThrow(
      "Invalid PEER_PERFORMANCE_MODE",
    );
    for (const value of [NaN, Infinity, 0, -1])
      expect(() =>
        assessPerformance(
          { scrollP95Ms: value, warmNavigationP95Ms: 67 },
          "report",
        ),
      ).toThrow("Invalid performance measurement");
  });
});
