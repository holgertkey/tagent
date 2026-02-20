import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { readFileSync } from "fs";
import { resolve } from "path";

const pkg = JSON.parse(
  readFileSync(resolve(__dirname, "package.json"), "utf-8")
);

export default defineConfig({
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    target: ["es2021", "chrome100", "safari13"],
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
