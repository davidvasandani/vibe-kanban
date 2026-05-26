import path from "path";
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: [
      {
        find: "@remote",
        replacement: path.resolve(__dirname, "src"),
      },
      {
        find: /^@\//,
        replacement: `${path.resolve(__dirname, "../web-core/src")}/`,
      },
      {
        find: "shared",
        replacement: path.resolve(__dirname, "../../shared"),
      },
    ],
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    css: false,
  },
});
