import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [
    svelte({
      compilerOptions: {
        runes: true,
      },
    }),
  ],
  base: "/local-asr-assets/",
  root: "src-local-asr",
  build: {
    target: "es2022",
    outDir: "../bin/local-asr",
    emptyOutDir: true,
  },
});
