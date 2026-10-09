import { defineConfig } from "vitest/config";
export default defineConfig({
  test: {
    include: ["src/**/*.test.ts", "tests/desktop/*.test.ts"],
    environment: "node",
  },
});
