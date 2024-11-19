import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import viteTsconfigPaths from 'vite-tsconfig-paths'

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), viteTsconfigPaths()],
  server: {},
  build: {
    rollupOptions: {
      makeAbsoluteExternalsRelative: true,
      output: {
        assetFileNames: "[name].[ext]",
        entryFileNames: "[name].js",
        chunkFileNames: "[name].js",
      } 
    }
  }
}));
