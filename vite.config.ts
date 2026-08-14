import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// El puerto es fijo porque tauri.conf.json apunta a él en desarrollo.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2022" },
});
