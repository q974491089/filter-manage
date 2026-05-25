import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { readFileSync } from "fs";

import { cloudflare } from "@cloudflare/vite-plugin";

const host = process.env.TAURI_DEV_HOST;
const pkg = JSON.parse(readFileSync("./package.json", "utf-8"));

export default defineConfig(async () => ({
  plugins: [react(), cloudflare()],
  clearScreen: false,
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
  optimizeDeps: {
    exclude: ["src-tauri"],
  },
  build: {
    // 排除 Rust 构建目录
    commonjsOptions: {
      exclude: ["src-tauri/**"],
    },
  },
  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**", "**/target/**"],
    },
    fs: {
      // 限制 Vite 只能访问项目根目录
      strict: true,
      allow: ["."],
    },
  },
}));