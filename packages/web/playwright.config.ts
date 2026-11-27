import { defineConfig, devices } from "@playwright/test";
import { env } from "node:process";

const browsers = env.HEARTHLINE_ALL_BROWSERS === "1"
  ? [
      { name: "chromium-desktop", use: { ...devices["Desktop Chrome"] } },
      { name: "firefox-desktop", use: { ...devices["Desktop Firefox"] } },
      { name: "webkit-desktop", use: { ...devices["Desktop Safari"] } },
      { name: "chromium-mobile", use: { ...devices["Pixel 7"] } },
      { name: "webkit-mobile", use: { ...devices["iPhone 15"] } },
    ]
  : [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }];

export default defineConfig({
  testDir: "./tests/e2e",
  outputDir: "artifacts/playwright/results",
  reporter: [["list"], ["html", { outputFolder: "artifacts/playwright/report", open: "never" }]],
  fullyParallel: true,
  forbidOnly: Boolean(env.CI),
  retries: env.CI ? 1 : 0,
  timeout: 30_000,
  expect: { timeout: 15_000 },
  use: {
    baseURL: "http://127.0.0.1:4173",
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },
  projects: browsers,
  webServer: [
    {
      command: "cargo run --locked --manifest-path ../Cargo.toml -p hearthline-api",
      url: "http://127.0.0.1:3001/api/health",
      timeout: 180_000,
      reuseExistingServer: !env.CI,
    },
    {
      command: `npm run ${env.CI ? "preview" : "dev"} -- --host 127.0.0.1 --port 4173 --strictPort`,
      url: "http://127.0.0.1:4173",
      timeout: 60_000,
      reuseExistingServer: !env.CI,
    },
  ],
});
