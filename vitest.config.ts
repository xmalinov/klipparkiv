import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    css: true,
    coverage: {
      provider: "v8",
      reporter: ["text", "html", "lcov"],
      include: ["src/**/*.{ts,tsx}"],
      exclude: ["src/main.tsx", "src/vite-env.d.ts", "src/test/**"],
      // Ratchet: keep a couple of points below the measured baseline and only raise.
      thresholds: {
        statements: 88,
        branches: 78,
        functions: 86,
        lines: 90,
      },
    },
  },
});
