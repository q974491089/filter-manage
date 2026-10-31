import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { readFileSync } from "fs";

const host = process.env.TAURI_DEV_HOST;
const pkg = JSON.parse(readFileSync("./package.json", "utf-8"));

export default defineConfig(async () => ({
  plugins: [react()],
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
    proxy: {
      // tauri dev 时前端 origin 是 localhost:5173，直连上传网关会被 CORS 预检
      // 拦截（Worker 白名单只有打包客户端的 tauri.localhost）。经 Vite 同源
      // 代理转发，开发期就是普通同源请求；打包后走 tauri.localhost，不受影响。
      "/v1": {
        target: "https://icc-upload.xyls.us.kg",
        changeOrigin: true,
      },
    },
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
