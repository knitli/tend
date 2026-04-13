import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";

// Tauri-aware config per https://tauri.app/v2/reference/config
export default defineConfig({
	plugins: [svelte()],

	resolve: {
		alias: {
			// SvelteKit auto-resolves $lib for .svelte files; this alias makes it
			// available in plain .ts test files for Vitest as well.
			$lib: new URL("./src/lib", import.meta.url).pathname,
		},
	},

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
});
