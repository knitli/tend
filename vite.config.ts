import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Tauri-aware config per https://tauri.app/v2/reference/config
export default defineConfig({
  plugins: [svelte()],

  // Prevent Vite from obscuring Rust errors
  clearScreen: false,

  // Tauri expects a fixed port, fail if not available
  server: {
    port: 1420,
    strictPort: true,
    host: "127.0.0.1",
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },

  // Env vars with this prefix are exposed to the frontend
  envPrefix: ["VITE_", "TAURI_"],

  build: {
    target: "esnext",
    minify: "esbuild",
    sourcemap: true,
  },

  test: {
    environment: "jsdom",
    globals: true,
  },
});
