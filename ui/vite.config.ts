import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

function isNodeModulePkg(id: string, name: string): boolean {
  return id.includes(`/node_modules/${name}/`) || id.includes(`\\node_modules\\${name}\\`);
}

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

            if (isNodeModulePkg(id, "react-day-picker") || isNodeModulePkg(id, "date-fns")) {
              return "date-vendor";
            }
            if (id.includes("/node_modules/@radix-ui/") || id.includes("\\node_modules\\@radix-ui\\")) {
              return "radix-vendor";
            }
            if (isNodeModulePkg(id, "lucide-react")) return "icons-vendor";
            if (
              isNodeModulePkg(id, "react") ||
              isNodeModulePkg(id, "react-dom") ||
              isNodeModulePkg(id, "scheduler") ||
              isNodeModulePkg(id, "react-is") ||
              isNodeModulePkg(id, "use-sync-external-store")
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
