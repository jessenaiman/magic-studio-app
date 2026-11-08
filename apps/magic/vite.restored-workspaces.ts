import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join, relative } from "node:path";
import type { Plugin } from "vite";

// Package the retained UI source directly. No second source copy and no HTTP engine server.
export function restoredWorkspaces(): Plugin {
  const source = fileURLToPath(new URL("../magic-cli/ui/", import.meta.url));
  const adapter = fileURLToPath(new URL("./ui/workspaces/tauri-adapter.js", import.meta.url));
  const files = new Map<string, string>();
  const walk = (directory: string) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) walk(path);
      else if (/\.(html|js|css|json|svg|png|gif|webp|jpg|jpeg|woff2?)$/.test(entry.name)) files.set(relative(source, path).replaceAll("\\", "/"), path);
    }
  };
  walk(source); files.set("tauri-adapter.js", adapter);
  files.delete("vendor/konva.bundle.js"); files.delete("vendor/rig-review.bundle.js");
  // Converted modules are emitted as chunks (see buildStart); the legacy sources must not be copied over them.
  files.delete("settings.js");
  files.set("animation-connection.js", fileURLToPath(new URL("./ui/workspaces/animation-connection.js", import.meta.url)));
  files.set("embedded.js", fileURLToPath(new URL("./ui/workspaces/embedded.js", import.meta.url)));
  files.set("project-workspace.html", join(source, "index.html"));
  files.set("animation-workspace.html", join(source, "index.html"));
  const content = (name: string) => {
    if (name === "theatre-frame.html") return Buffer.from('<!doctype html><script>location.replace("/theatre.html")</script>');
    let data = readFileSync(files.get(name)!);
    if (name.endsWith(".html")) {
      let html = data.toString().replace('<script src="vendor/konva.bundle.js"></script>', '<script type="module" src="/workspaces/vendor.js"></script>')
        .replace('<script src="vendor/rig-review.bundle.js"></script>', '');
      if (name === "project-workspace.html" || name === "animation-workspace.html") html = html.replace('</head>', '<style>#app-bar,#workspace-select{display:none!important}body{height:100vh;grid-template-rows:minmax(0,1fr)!important}body>main{height:100%;min-height:0}.workshop-drawers{top:0}</style></head>')
        .replace('</body>', '<script src="animation-connection.js"></script><script src="embedded.js"></script></body>');
      data = Buffer.from(html);
    }
    return data;
  };
  let building = false;
  return {
    configResolved(config) { building = config.command === "build"; },
    name: "restored-workspaces",
    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        const path = new URL(request.url || "/", "http://localhost").pathname;
        if (!path.startsWith("/workspaces/")) return next();
        const name = path.slice("/workspaces/".length);
        if (name === "vendor.js") { response.statusCode = 302; response.setHeader("Location", "/ui/workspaces/vendor.ts"); response.end(); return; }
        if (name === "settings.js") { response.statusCode = 302; response.setHeader("Location", "/ui/workspaces/legacy/settings/main.tsx"); response.end(); return; }
        if (!files.has(name)) return next();
        const type = name.endsWith(".js") ? "text/javascript" : name.endsWith(".css") ? "text/css" : name.endsWith(".html") ? "text/html" : name.endsWith(".json") ? "application/json" : name.endsWith(".svg") ? "image/svg+xml" : name.endsWith(".gif") ? "image/gif" : name.endsWith(".png") ? "image/png" : "application/octet-stream";
        response.setHeader("Content-Type", type); response.setHeader("Cache-Control", "no-store"); response.end(content(name));
      });
    },
    buildStart() {
      if (!building) return;
      this.emitFile({ type: "chunk", id: fileURLToPath(new URL("./ui/workspaces/vendor.ts", import.meta.url)), fileName: "workspaces/vendor.js" });
      this.emitFile({ type: "chunk", id: fileURLToPath(new URL("./ui/workspaces/legacy/settings/main.tsx", import.meta.url)), fileName: "workspaces/settings.js" });
    },
    generateBundle() {
      for (const name of files.keys()) this.emitFile({ type: "asset", fileName: `workspaces/${name}`, source: content(name) });
    },
  };
}
