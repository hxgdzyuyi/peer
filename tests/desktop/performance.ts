export type PerformanceMode = "enforce" | "report";
export type PerformanceMetrics = {
  scrollP95Ms: number;
  warmNavigationP95Ms: number;
};

export function performanceMode(value = "enforce"): PerformanceMode {
  if (value !== "enforce" && value !== "report")
    throw new Error(`Invalid PEER_PERFORMANCE_MODE: ${value}`);
  return value;
}

export function assessPerformance(
  metrics: PerformanceMetrics,
  mode: PerformanceMode,
) {
  const budgets = { scrollP95Ms: 33, warmNavigationP95Ms: 100 };
  const violations: string[] = [];
  for (const key of Object.keys(budgets) as (keyof PerformanceMetrics)[]) {
    const value = metrics[key];
    if (!Number.isFinite(value) || value <= 0)
      throw new Error(`Invalid performance measurement: ${key}=${value}`);
    if (value >= budgets[key])
      violations.push(`${key}: ${value} ms (target < ${budgets[key]} ms)`);
  }
  return {
    mode,
    status: violations.length ? "over-budget" : "within-budget",
    budgets,
    violations,
  };
}

export function enforcePerformance(
  report: ReturnType<typeof assessPerformance>,
) {
  if (report.mode === "enforce" && report.violations.length)
    throw new Error(
      `Performance budget exceeded: ${report.violations.join("; ")}`,
    );
}
