import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  build: {
    target: "es2022",
    outDir: "bin/dashboard",
    emptyOutDir: true,
  },
  server: {
    strictPort: true,
    port: 5173,
  },
});
