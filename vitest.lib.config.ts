import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: [
      "src/lib/**/*.test.ts",
      "src/lib/i18n/**/*.test.ts",
      "src-tts/lib/**/*.test.ts",
    ],
    setupFiles: ["./vitest.lib.setup.ts"],
  },
});
