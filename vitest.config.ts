import { defineConfig, mergeConfig } from "vitest/config";
import viteConfig from "./vite.config";

export default mergeConfig(viteConfig, defineConfig({
    test: {
        environment: "jsdom",
        globals: true,
        exclude: ["tests/e2e/**"],
        include: ["src/**/*.{test,spec}.{ts,tsx}"],
    },
}));
