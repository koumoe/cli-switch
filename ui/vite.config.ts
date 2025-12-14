import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig(() => {
  return {
    plugins: [react()],
    build: {
      outDir: "dist",
      sourcemap: true
    },
    server: {
      proxy: {
        "/api": {
          target: process.env.VITE_BACKEND_URL ?? "http://127.0.0.1:3210",
          changeOrigin: true
        }
      }
    }
  };
});

