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
  base: "/tts-assets/",
  root: "src-tts",
  build: {
    target: "es2022",
    outDir: "../bin/tts",
    emptyOutDir: true,
  },
});
