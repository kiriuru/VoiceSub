import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "happy-dom",
    include: ["tests/renderer/**/*.test.ts"],
    setupFiles: ["./tests/renderer/setup.ts"],
  },
});
