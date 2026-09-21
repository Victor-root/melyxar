import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

/// What comes out of here is baked into the server's own binary, so deploying
/// the server is deploying one file and nothing beside it.
///
/// It is never kept in the repository, on purpose: a built interface sitting
/// next to the sources it was built from goes stale the moment anybody edits
/// one of them, and nothing would say so. The update script builds it again
/// just before it compiles the server, which is the only order in which the
/// two can agree.
export default defineConfig({
  plugins: [react()],
  build: {
    outDir: "dist",
    emptyOutDir: true,
    // Named without a hash: the server hands these over with a short cache
    // and the pictures, which never change under one name, with a long one.
    assetsDir: "assets",
    // Where the builder starts advising a split. Raised above its own default
    // because at that default it advises splitting what is already split: the
    // reader of streams is five hundred and seventy kilobytes and is fetched
    // only when a film is played, never on the home page, which is exactly
    // the arrangement the advice asks for. What is left is the interface
    // itself at around a hundred and fifty five kilobytes compressed, handed
    // over by the same machine on the same network as the films. The number
    // is here so that a chunk which really did run away would still be
    // caught.
    chunkSizeWarningLimit: 700,
  },
  server: {
    port: 5173,
    proxy: {
      "/api": "http://localhost:2100",
    },
  },
});
