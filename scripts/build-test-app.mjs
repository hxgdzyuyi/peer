import { spawnSync } from "node:child_process";
const child = spawnSync(
  process.platform === "win32" ? "npm.cmd" : "npm",
  [
    "run",
    "tauri",
    "build",
    "--",
    "--debug",
    "--no-bundle",
    "--features",
    "desktop-tests",
    "--config",
    "src-tauri/tauri.test.conf.json",
  ],
  { stdio: "inherit", env: { ...process.env, VITE_DESKTOP_TEST: "1" } },
);
process.exit(child.status ?? 1);
