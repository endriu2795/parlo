import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
export default defineConfig({
  plugins: [react()],
  optimizeDeps: { entries: ["index.html", "overlay.html"] },
  build: { rollupOptions: { input: ["index.html", "overlay.html"] } },
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/.tools/**", "**/.local-data/**", "**/src-tauri/**"] },
  },
  clearScreen: false,
});
