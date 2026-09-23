import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

/// What comes out of here is put by the update script in the folder the
/// server reads the interface from, and served from the next request on.
///
/// It is never kept in the repository, on purpose: a built interface sitting
/// next to the sources it was built from goes stale the moment anybody edits
/// one of them, and nothing would say so. The update script builds it again
/// whenever something under web/ changed, and puts it in place only once the
/// server beside it is ready, so the two always agree.
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
