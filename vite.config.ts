import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The port is fixed because tauri.conf.json points to it during development.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2022" },
});
