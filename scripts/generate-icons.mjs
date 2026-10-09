import { copyFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const root = fileURLToPath(new URL("../", import.meta.url));
const output = mkdtempSync(join(tmpdir(), "peer-icons-"));
try {
  const child = spawnSync(
    process.platform === "win32" ? "npm.cmd" : "npm",
    ["exec", "--", "tauri", "icon", "assets/icon.svg", "--output", output],
    { cwd: root, stdio: "inherit" },
  );
  if (child.error) throw child.error;
  if (child.status !== 0) throw new Error("Icon generation failed");
  // Keep the desktop assets in sync with the SVG used inside the app.
  for (const name of [
    "32x32.png",
    "64x64.png",
    "128x128.png",
    "128x128@2x.png",
    "icon.png",
    "icon.icns",
    "icon.ico",
  ]) {
    copyFileSync(join(output, name), join(root, "src-tauri/icons", name));
  }
} finally {
  rmSync(output, { recursive: true, force: true });
}
