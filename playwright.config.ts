import { defineConfig } from '@playwright/test';

/**
 * Playwright config for E2E tests.
 *
 * Two modes of operation:
 *
 * 1. **Dev server mode** (default): Tests run against `pnpm tauri dev`
 *    (Vite dev server at localhost:1420). Start the dev server before
 *    running tests. Invoke calls go through the Tauri IPC bridge.
 *
 * 2. **tauri-driver mode** (CI): Requires:
 *    - `cargo install tauri-driver`
 *    - A debug build: `pnpm tauri build --debug`
 *    - WebKitGTK on the system
 *    - A custom fixture that launches tauri-driver and connects via
 *      WebDriver. See Tauri E2E docs for the fixture setup.
 *
 * Run with: pnpm e2e
 */
export default defineConfig({
  testDir: './tests/e2e',
  timeout: 30_000,
  retries: 0,
  use: {
    baseURL: 'http://localhost:1420',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },
  // Start the Vite dev server automatically if not already running.
  // For tauri-driver mode, comment this out and use a custom fixture.
  webServer: {
    command: 'pnpm dev',
    url: 'http://localhost:1420',
    reuseExistingServer: true,
    timeout: 30_000,
  },
  projects: [
    {
      name: 'tauri',
      testMatch: '**/*.spec.ts',
    },
  ],
});
