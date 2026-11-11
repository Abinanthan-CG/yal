import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

// The palette is a static page: Tauri serves `dist` from the webview, so the
// build has to land in `frontend/dist` and use relative paths.
export default defineConfig({
  plugins: [solid()],
  // The sources live in `src`, and the built page lands in `frontend/dist`,
  // where Tauri expects it.
  root: "src",
  base: "./",
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    target: "chrome110",
  },
  server: {
    port: 5173,
    strictPort: true,
  },
});
