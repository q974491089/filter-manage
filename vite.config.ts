import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react()],
  clearScreen: false,
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
    port: 1420,
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
