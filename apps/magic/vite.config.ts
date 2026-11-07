import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { restoredWorkspaces } from "./vite.restored-workspaces.ts";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [react(), restoredWorkspaces()],
  clearScreen: false,
  build: {
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: { input: { main: "index.html", theatre: "theatre.html" } },
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
      ignored: ["**/src/**", "**/target/**"],
    },
  },
}));
