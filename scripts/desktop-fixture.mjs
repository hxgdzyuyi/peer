import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execFileSync } from "node:child_process";

export function createFixture({ linesPerFile = 100 } = {}) {
  const root = mkdtempSync(join(tmpdir(), "peer-e2e-"));
  const repo = join(root, "benchmark-repo");
  const data = join(root, "app-data");
  mkdirSync(join(repo, "src"), { recursive: true });
  mkdirSync(data);
  const git = (...args) =>
    execFileSync("git", args, { cwd: repo, stdio: "pipe" });
  git("init", "-b", "main");
  git("config", "user.name", "Peer Test");
  git("config", "user.email", "test@example.invalid");
  writeFileSync(
    join(repo, ".editorconfig"),
    "root = true\n[*]\nindent_style = tab\ntab_width = 4\n",
  );
  writeFileSync(
    join(repo, "notes.txt"),
    Array.from({ length: 100 }, (_, i) => `Plain text line ${i + 1}\n`).join(
      "",
    ),
  );
  const source = (index, changed) =>
    Array.from(
      { length: linesPerFile },
      (_, line) =>
        `export const item${index}_${line} = {\tvalue: ${changed && line < 100 && line % 2 ? line + 1000 : line}, label: "entry ${line}" };  \n`,
    ).join("");
  for (let i = 0; i < 100; i++)
    writeFileSync(
      join(repo, "src", `module-${String(i).padStart(3, "0")}.ts`),
      source(i, false),
    );
  git("add", ".");
  git("commit", "-m", "Baseline");
  for (let i = 0; i < 100; i++)
    writeFileSync(
      join(repo, "src", `module-${String(i).padStart(3, "0")}.ts`),
      source(i, true),
    );
  writeFileSync(
    join(data, "state.json"),
    JSON.stringify({
      schemaVersion: 1,
      preferences: { lastRepository: repo },
      queues: {},
      themes: [],
    }),
  );
  return { root, repo, data };
}
