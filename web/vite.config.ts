import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

/// The built interface is committed and embedded in the server binary, so the
/// container it runs in needs no JavaScript tooling of its own.
export default defineConfig({
  plugins: [react()],
  build: {
    outDir: "dist",
    emptyOutDir: true,
    // Named without a hash: the server hands these over with a short cache
    // and the pictures, which never change under one name, with a long one.
    assetsDir: "assets",
  },
  server: {
    port: 5173,
    proxy: {
      "/api": "http://localhost:2100",
    },
  },
});
