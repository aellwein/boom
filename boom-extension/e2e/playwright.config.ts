import { defineConfig } from "@playwright/test";

// Each test starts its own Chromium with the built extension (dist/), so the
// tests are isolated. Run `npm run e2e` to build first.
export default defineConfig({
  testDir: ".",
  testMatch: "*.spec.ts",
  timeout: 120_000,
  expect: { timeout: 30_000 },
  workers: 1,
  retries: 0,
  reporter: [["list"]],
  use: {
    trace: "retain-on-failure",
  },
});
