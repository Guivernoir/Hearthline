import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  resolve: { conditions: ["browser"] },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts"],
    fileParallelism: false,
    maxWorkers: 1,
    coverage: {
      provider: "v8",
      reporter: ["text", "json-summary", "html"],
      reportsDirectory: "artifacts/coverage/web",
      include: ["src/lib/**/*.{ts,svelte}"],
      exclude: ["src/generated/**", "src/lib/shared/types.ts"],
      thresholds: {
        lines: 75,
        branches: 70,
      },
    },
  },
});
