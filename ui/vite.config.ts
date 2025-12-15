import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

export default defineConfig(() => {
  const sourcemap = process.env.VITE_SOURCEMAP === "true";
  return {
    plugins: [react()],
    resolve: {
      alias: {
        "@": path.resolve(__dirname, "./src"),
      },
    },
    build: {
      outDir: "dist",
      sourcemap,
    },
    server: {
      proxy: {
        "/api": {
          target: process.env.VITE_BACKEND_URL ?? "http://127.0.0.1:3210",
          changeOrigin: true
        },
        "/v1": {
          target: process.env.VITE_BACKEND_URL ?? "http://127.0.0.1:3210",
          changeOrigin: true
        }
      }
    }
  };
});
