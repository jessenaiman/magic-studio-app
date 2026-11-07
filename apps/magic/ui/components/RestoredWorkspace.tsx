import React, { useEffect, useId, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import { listen } from "@tauri-apps/api/event";
import { executeCommand } from "../api";

interface Host {
  navigate: (view: string) => Promise<unknown>;
  invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>;
  fileUrl: typeof convertFileSrc; openDialog: typeof open; openPath: typeof openPath; openUrl: typeof openUrl; listen: typeof listen;
  report: (message: string) => void;
}
declare global { interface Window { __MAGIC_WORKSPACE_HOSTS__?: Map<string, Host> } }
interface Props { project: string; page: string; title: string; onToast: (message: string) => void }
export const RestoredWorkspace: React.FC<Props> = ({ project, page, title, onToast }) => {
  const id = `restored-${useId().replace(/[^a-z0-9]/gi, "")}`;
  const [ready, setReady] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    const registry: Map<string, Host> = window.__MAGIC_WORKSPACE_HOSTS__ ||= new Map<string, Host>();
    registry.set(id, {
      navigate: view => executeCommand("ui.set", { view }),
      invoke: async (command, args = {}) => {
        if (!active) throw new Error("The workspace has changed");
        if (command === "save_animation_export") {
          const bytes = ArrayBuffer.isView(args) ? new Uint8Array(args.buffer, args.byteOffset, args.byteLength) : Array.isArray(args) ? new Uint8Array(args) : null;
          if (!bytes || bytes.length < 12 || String.fromCharCode(...bytes.slice(0, 4)) !== "glTF") throw new Error("Animation export did not return a GLB");
          const folder = project.replace(/[/\\][^/\\]+$/, "");
          const path = await save({ title: "Export revised animation", defaultPath: `${folder}/exports/animation-revised.glb`, filters: [{ name: "Animated GLB", extensions: ["glb"] }] });
          if (!path) throw new Error("Export cancelled");
          if (!active) throw new Error("Project changed before export; nothing was written");
          const assets = await executeCommand<{ assets: Array<{ path?: string }> }>("project.list_assets");
          if (assets.assets.some(asset => asset.path === path)) throw new Error("Choose a new file to preserve the registered source model");
          await writeFile(path, bytes);
          return path;
        }
        if (command === "execute_command") {
          const params = args.params && typeof args.params === "object" ? args.params : {};
          return executeCommand(String(args.id), { ...params, _expected_project: project });
        }
        // Only the existing Tauri commands are exposed; unknown native calls fail visibly.
        return invoke(command, args);
      },
      fileUrl: convertFileSrc, openDialog: open, openPath, openUrl, listen,
      report: message => { if (active) { setError(message); onToast(message); } },
    });
    setReady(true);
    return () => { active = false; registry.delete(id); };
  }, [id, project]);
  return <section id={`restored-${page.replace(/\.html$/, "")}`} style={{ height: "100%", minHeight: 0, display: "flex", flexDirection: "column" }}>
    {error && <div role="alert" style={{ color: "#ed8796", padding: "6px 12px", background: "#281b24", fontSize: 12 }}>{error}</div>}
    {ready && <iframe id={id} title={title} src={`/workspaces/${page}`} style={{ position: "relative", inset: "auto", border: 0, width: "100%", minHeight: 0, flex: 1 }} />}
  </section>;
};
