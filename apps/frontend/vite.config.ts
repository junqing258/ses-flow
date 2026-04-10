/// <reference types="vitest/config" />
import tailwindcss from "@tailwindcss/vite";
import vue from "@vitejs/plugin-vue";
import { codeInspectorPlugin } from "code-inspector-plugin";
import { defineConfig } from "vite";

export default defineConfig(() => {
  const isVitest = process.env.VITEST === "true";

  return {
    plugins: [
      tailwindcss(),
      vue(),
      ...(!isVitest
        ? [
          codeInspectorPlugin({
            bundler: "vite",
            // editor: "code", // 指定 IDE 为 vscode
          }),
        ]
        : []),
    ],
    resolve: {
      tsconfigPaths: true,
    },
    build: {
      cache: {
        enabled: true,
      },
    },
    server: {
      host: "0.0.0.0",
      proxy: {
        "/api": {
          target: process.env.VITE_API_PROXY_TARGET ?? "http://127.0.0.1:3000",
          changeOrigin: true,
        },
        "/runner-api": {
          target: process.env.VITE_RUNNER_PROXY_TARGET ?? "http://127.0.0.1:3002",
          changeOrigin: true,
          rewrite: (path) => path.replace(/^\/runner-api/, ""),
        },
      },
    },
    test: {
      environment: "node",
      include: ["tests/**/*.test.ts"],
    },
  };
});
