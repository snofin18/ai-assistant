/**
 * Vitest configuration for DOM-level component tests.
 *
 * Pure model tests stay on `node --test` (`*.test.mjs`); React component and
 * interaction tests live in `*.test.tsx` and run here under jsdom.
 */

import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.tsx"],
    // Testing Library registers its own cleanup on the global `afterEach`, so
    // the globals must exist or the DOM leaks between tests.
    globals: true,
    restoreMocks: true,
  },
});
