import type { SidebarWidths } from "./types";

export const sidebarLimits = {
  files: { min: 180, max: 600 },
  feedback: { min: 260, max: 640 },
};
export const minimumSourceWidth = 360;

export function clampSidebar(side: keyof SidebarWidths, value: number) {
  const { min, max } = sidebarLimits[side];
  return Math.round(Math.max(min, Math.min(max, value)));
}

/** Fit the current window without changing the user's saved widths. */
export function fitSidebars(
  preferred: SidebarWidths,
  width: number,
  showRight: boolean,
): SidebarWidths {
  const files = clampSidebar("files", preferred.files);
  const feedback = clampSidebar("feedback", preferred.feedback);
  const budget = Math.max(
    sidebarLimits.files.min + (showRight ? sidebarLimits.feedback.min : 0),
    width - minimumSourceWidth,
  );
  if (!showRight) return { files: Math.min(files, budget), feedback };
  const excess = Math.max(0, files + feedback - budget);
  const fileRoom = files - sidebarLimits.files.min;
  const feedbackRoom = feedback - sidebarLimits.feedback.min;
  const shrinkFiles =
    fileRoom + feedbackRoom > 0
      ? Math.min(
          fileRoom,
          Math.round((excess * fileRoom) / (fileRoom + feedbackRoom)),
        )
      : 0;
  return {
    files: files - shrinkFiles,
    feedback: feedback - (excess - shrinkFiles),
  };
}
