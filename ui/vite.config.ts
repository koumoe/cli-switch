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
      rollupOptions: {
        output: {
          manualChunks(id) {
            if (!id.includes("node_modules")) return;

            const isPkg = (name: string) => id.includes(`/node_modules/${name}/`) || id.includes(`\\node_modules\\${name}\\`);

            if (isPkg("react-day-picker") || isPkg("date-fns")) {
              return "date-vendor";
            }
            if (id.includes("/node_modules/@radix-ui/") || id.includes("\\node_modules\\@radix-ui\\")) {
              return "radix-vendor";
            }
            if (isPkg("lucide-react")) return "icons-vendor";
            if (
              isPkg("react") ||
              isPkg("react-dom") ||
              isPkg("scheduler") ||
              isPkg("react-is") ||
              isPkg("use-sync-external-store")
            ) {
              return "react-vendor";
            }
            return "vendor";
          },
        },
      },
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
