import React, { useEffect, useId, useRef, useState } from "react";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import { openPath, openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { listen } from "@tauri-apps/api/event";
import { clear, readImage, readText, writeHtml, writeImage, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { executeCommand } from "../api";
import { takeAuthoringTime } from "../authoring-navigation";

interface Host {
  takeReviewTime: () => number | undefined;
  isActive: () => boolean;
  authoringMode: () => string | undefined;
  navigate: (view: string) => Promise<unknown>;
  invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>;
  fileUrl: typeof convertFileSrc; openDialog: typeof open; saveDialog: typeof save; openPath: typeof openPath; openUrl: typeof openUrl; revealItemInDir: typeof revealItemInDir; listen: typeof listen;
  clipboard: { readText: typeof readText; writeText: typeof writeText; readImage: typeof readImage; writeImage: typeof writeImage; writeHtml: typeof writeHtml; clear: typeof clear };
  report: (message: string) => void;
}
declare global { interface Window { __MAGIC_WORKSPACE_HOSTS__?: Map<string, Host> } }
interface Props { project: string; page: string; title: string; active?: boolean; authoringMode?: string; onToast: (message: string) => void }
export const RestoredWorkspace: React.FC<Props> = ({ project, page, title, active: visible = true, authoringMode, onToast }) => {
  const id = `restored-${useId().replace(/[^a-z0-9]/gi, "")}`;
  const frame = useRef<HTMLIFrameElement>(null);
  const modeRef = useRef(authoringMode); modeRef.current = authoringMode;
  useEffect(() => { frame.current?.contentWindow?.postMessage({ magicAuthoringMode: authoringMode }, location.origin); }, [authoringMode]);
  const activeView = useRef(visible); activeView.current = visible;
  useEffect(() => { frame.current?.contentWindow?.postMessage({ magicWorkspaceVisibility: true, active: visible }, location.origin); }, [visible]);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    let active = true;
    const registry: Map<string, Host> = window.__MAGIC_WORKSPACE_HOSTS__ ||= new Map<string, Host>();
    registry.set(id, {
      takeReviewTime: () => page==="lyrics.html"?takeAuthoringTime(project):undefined,
      isActive: () => activeView.current,
      authoringMode: () => modeRef.current,
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
      fileUrl: convertFileSrc, openDialog: open, saveDialog: save, openPath, openUrl, revealItemInDir, listen,
      clipboard: { readText, writeText, readImage, writeImage, writeHtml, clear },
      report: message => { if (active) { setError(message); onToast(message); void invoke("record_ui_error", { message, source: page, control: id }).catch(() => {}); } },
    });
    setReady(true);
    return () => { active = false; registry.delete(id); };
  }, [id, project]);
  return <section id={`restored-${page.replace(/\.html$/, "")}`} data-workspace-error={error || undefined} className="restored-workspace">
    {error && <div role="alert" className="restored-workspace-error">{error}</div>}
    {ready && <iframe ref={frame} id={id} onLoad={() => { frame.current?.contentWindow?.postMessage({ magicWorkspaceVisibility: true, active: visible }, location.origin); frame.current?.contentWindow?.postMessage({ magicAuthoringMode: modeRef.current }, location.origin); }} title={title} src={`/workspaces/${page}`} className="restored-workspace-frame" />}
  </section>;
};

const PAGES: Record<string, [string, string]> = {
  project: ["project-workspace.html", "Project workspace"],
  animation: ["animation-workspace.html", "Animation workspace"],
  design: ["lyrics.html", "Lyrics designer"],
};
// Keep visited editors mounted so switching views preserves their unsaved drafts and selection.
export function PreservedWorkspaces({ project, view, onToast }: { project: string; view: string; onToast: (message: string) => void }) {
  const authoring = ["design", "motion"].includes(view);
  const pageView = authoring ? "design" : view;
  const [opened, setOpened] = useState<string[]>(PAGES[pageView] ? [pageView] : []);
  useEffect(() => { if (PAGES[pageView]) setOpened(previous => previous.includes(pageView) ? previous : [...previous, pageView]); }, [pageView]);
  const shown = PAGES[pageView] && !opened.includes(pageView) ? [...opened, pageView] : opened;
  return <div className="restored-page"  hidden={!PAGES[pageView]}>
    {shown.map(mode => <div key={mode} className="restored-page"  hidden={mode !== pageView}>
      <RestoredWorkspace project={project} page={PAGES[mode][0]} title={mode === "design" ? "Lyrics and Motion" : PAGES[mode][1]} active={mode === pageView} authoringMode={mode === "design" ? (authoring ? view : undefined) : undefined} onToast={onToast} />
    </div>)}
  </div>;
}
