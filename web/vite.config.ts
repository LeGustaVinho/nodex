import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// During development the Rust server runs on NODEX_PORT (default 7420) and must be started with
// `--dev-origin http://localhost:5173`. The proxy rewrites Host so the server's DNS-rebinding
// check accepts the request.
const target = `http://127.0.0.1:${process.env.NODEX_PORT ?? "7420"}`;

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    strictPort: true,
    proxy: {
      "/api": { target, changeOrigin: true },
      "/ws": { target, changeOrigin: true, ws: true },
    },
  },
  build: {
    chunkSizeWarningLimit: 4096,
  },
  test: {
    environment: "node",
  },
});
