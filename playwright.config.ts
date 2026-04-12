import { defineConfig } from '@playwright/test';

/**
 * Playwright config for E2E tests via tauri-driver.
 *
 * These tests require:
 *   1. `cargo install tauri-driver`
 *   2. A built Tauri app (`pnpm tauri build --debug`)
 *   3. WebKitGTK available on the system
 *
 * Run with: pnpm e2e
 */
export default defineConfig({
  testDir: './tests/e2e',
  timeout: 30_000,
  retries: 0,
  use: {
    // tauri-driver exposes a WebDriver endpoint; Playwright connects via CDP.
    // The baseURL and launch setup are handled per-test via the custom fixture.
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },
  projects: [
    {
      name: 'tauri',
      testMatch: '**/*.spec.ts',
    },
  ],
});
