/// <reference types="vitest/config" />
import tailwindcss from "@tailwindcss/vite";
import vue from "@vitejs/plugin-vue";
import { codeInspectorPlugin } from "code-inspector-plugin";
import Markdown from "unplugin-vue-markdown/vite";
import { defineConfig } from "vite";
import Pages from "vite-plugin-pages";

export default defineConfig(() => {
  const isVitest = process.env.VITEST === "true";

  return {
    base: "/views/",
    plugins: [
      tailwindcss(),
      vue({
        include: [/\.vue$/, /\.md$/],
      }),
      Pages({
        dirs: [{ dir: "src/routes", baseRoute: "" }],
        exclude: ['**/component(s)?/**/*.(vue|ts|tsx|md)', '**/component(s)?/*.(vue|ts|tsx|md)'],
        extensions: ['vue', 'tsx', 'md'],
      }),
      Markdown({
        markdownOptions: {
          html: true,
          linkify: true,
          typographer: true,
        },
        wrapperClasses: "app-markdown",
      }),
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
        },
      },
    },
    test: {
      environment: "node",
      include: ["tests/**/*.test.ts"],
    },
  };
});
