import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// In the desktop app the solver runs inside the Tauri process. In a normal browser (`npm run dev:web`,
// the click tests, the web demo) it runs as WebAssembly in a worker (src/ipc/solver.worker.ts), built
// by `npm run build:wasm`. The example models come from ../tests/benchmarks.
//
// `--mode web` (`npm run build:web`) is the web demo's static build: plain files in dist-web/ for the
// site path in OSAB_BASE (default /osab/, as on gifariz.com/osab/). The desktop build
// (`npm run build`, dist/, used by Tauri) stays at /.
// The web demo counts visits with the site host's web analytics (its script at /_vercel/insights/, which
// exists on the published site only, so local test servers do not load it).
const analytics = `<script>
      window.va = window.va || function () { (window.vaq = window.vaq || []).push(arguments); };
      if (!["localhost", "127.0.0.1", "[::1]"].includes(location.hostname)) {
        const s = document.createElement("script");
        s.defer = true;
        s.src = "/_vercel/insights/script.js";
        document.head.appendChild(s);
      }
    </script>`;

export default defineConfig(({ mode }) => {
  const web = mode === "web";
  return {
    plugins: [react(), ...(web ? [{ name: "web-analytics", transformIndexHtml: (html: string) => html.replace("</head>", `  ${analytics}
  </head>`) }] : [])],
    base: web ? (process.env.OSAB_BASE ?? "/osab/") : "/",
    clearScreen: false,
    server: { port: 5173, strictPort: true, fs: { allow: [".", "../tests/benchmarks"] } },
    preview: { port: 4173, strictPort: true },
    build: { target: "es2022", chunkSizeWarningLimit: 1500, outDir: web ? "dist-web" : "dist" },
    worker: { format: "es" },
    test: { include: ["src/**/*.test.ts"] },
  };
});
