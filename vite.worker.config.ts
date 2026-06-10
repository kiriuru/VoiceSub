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
  base: "/worker-assets/",
  root: "src-worker",
  build: {
    outDir: "../bin/worker",
    emptyOutDir: true,
  },
});
