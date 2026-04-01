/// <reference types="vitest/config" />
import { fileURLToPath, URL } from "node:url";

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
      alias: {
        "@": fileURLToPath(new URL("./src", import.meta.url)),
      },
    },
    server: {
      host: "0.0.0.0",
      proxy: {
        "/api": {
          target: process.env.VITE_API_PROXY_TARGET ?? "http://127.0.0.1:3000",
          changeOrigin: true,
        },
      },
    },
    test: {
      environment: "node",
      include: ["tests/**/*.test.ts"],
    },
  };
});
