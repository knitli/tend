import { defineConfig, mergeConfig } from "vitest/config";
import viteConfig from "./vite.config";

export default mergeConfig(viteConfig, defineConfig({
    // Svelte 5 `mount(...)` is only available in the client/browser build.
    // Without the `browser` condition, vite-plugin-svelte resolves the
    // server-side entry point and tests fail with lifecycle_function_unavailable.
    // Keep Vite/Vitest's default client conditions and ensure `browser`
    // remains present so vite-plugin-svelte resolves the browser entry.
    resolve: {
        conditions: ["module", "browser", "development|production"],
    },
    test: {
        environment: "jsdom",
        globals: true,
        exclude: ["tests/e2e/**"],
        include: ["src/**/*.{test,spec}.{ts,tsx}"],
    },
}));
