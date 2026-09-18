import { defineConfig } from "vitest/config";

// Kept separate from vite.config.ts: that config's server block is tuned for
// `tauri dev` (fixed port, ignored watch paths) and has nothing to do with
// running unit tests, so there's no benefit to merging them.
export default defineConfig({
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts"],
  },
});
