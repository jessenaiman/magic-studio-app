import React, { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { executeCommand, fileUrl, pickFile, ProjectAsset, StoryboardData, StoryboardPage } from "../api";
import "../storyboard.css";

interface Props {
  projectPath: string;
  storyboard?: StoryboardData;
  onRefresh: () => Promise<void>;
  onToast: (message: string) => void;
}

const StageAssets: React.FC<{ onToast: (message: string) => void }> = ({ onToast }) => {
  const [assets, setAssets] = useState<ProjectAsset[]>([]);
  const [tab, setTab] = useState<"characters" | "backgrounds">("characters");
  const load = useCallback(async () => {
    try {
      const snapshot = await invoke<{ data?: { assets?: { assets?: ProjectAsset[] } } }>("session_snapshot");
      setAssets(snapshot.data?.assets?.assets ?? []);
    } catch (cause) {
      onToast(String(cause));
    }
  }, [onToast]);
  useEffect(() => {
    void load();
  }, [load]);
  const backgrounds = assets.filter((asset) => asset.role === "background");
  const characters = assets.filter((asset) => asset.role !== "background");
  const visible = tab === "backgrounds" ? backgrounds : characters;
  const subtitle = (asset: ProjectAsset) =>
    asset.role === "background" ? "Background · Storyboard →" : asset.kind === "model_3d" ? "3D Rig · Animate →" : "Cutout · Animate →";
  const run = async (fn: () => Promise<unknown>, refresh = false) => {
    try {
      await fn();
      if (refresh) await load();
    } catch (cause) {
      onToast(String(cause));
    }
  };
  return (
    <section id="stage-assets-panel" className="sb-stage" aria-label="Stage Assets & Animate">
      <div className="sb-stage-bar">
        <strong>Stage Assets &amp; Animate <span className="muted">({assets.length})</span></strong>
        <button id="stage-tab-characters" className={tab === "characters" ? "start" : "ghost"} onClick={() => setTab("characters")}>Characters / Animate ({characters.length})</button>
        <button id="stage-tab-plates" className={tab === "backgrounds" ? "start" : "ghost"} onClick={() => setTab("backgrounds")}>Backgrounds ({backgrounds.length})</button>
        <button id="stage-add-asset" className="ghost sb-stage-add" onClick={() => void run(async () => {
          const selected = await pickFile({ title: "Add Asset", extensions: ["png", "jpg", "jpeg", "webp", "gif", "svg", "glb", "gltf"] });
          if (typeof selected === "string" && selected) {
            const name = (selected.split(/[/\\]/).pop() || selected).replace(/\.[^.]+$/, "");
            const destination = tab === "backgrounds" ? { background: name } : { image: name };
            const added = await executeCommand<{ asset_id: string }>("project.add_asset", { path: selected, name, ...destination, load: false });
            onToast(`Added ${added.asset_id}`);
          }
        }, true)}>+ Add Asset</button>
      </div>
      <div className="sb-stage-row">
        {visible.map((asset) => (
          <div
            key={asset.id}
            className="sb-stage-card"
            role="button"
            tabIndex={0}
            title={asset.name || asset.id}
            onClick={() => void run(async () => {
              await executeCommand("character.open", { id: asset.id });
              await executeCommand("ui.set", { view: "animation" });
            })}
          >
            <button
              id={`stage-asset-remove-${asset.id}`}
              className="ghost sb-stage-remove"
              aria-label={`Remove ${asset.name || asset.id}`}
              title={`Remove ${asset.name || asset.id} from project`}
              onClick={(event) => {
                event.stopPropagation();
                void run(() => executeCommand("project.remove_asset", { id: asset.id }), true);
              }}
            >×</button>
            {/\.(png|jpe?g|webp|gif|svg)$/i.test(asset.path || "")
              ? <img className="sb-stage-thumb" src={fileUrl(asset.path)} alt="" />
              : <div className="muted sb-stage-placeholder">{asset.kind === "model_3d" ? "3D RIG" : "PART"}</div>}
            <div className="sb-stage-meta">
              <div className="sb-stage-name">{asset.name || asset.id}</div>
              <div className="muted sb-stage-sub">{subtitle(asset)}</div>
            </div>
          </div>
        ))}
        {!visible.length && <p className="muted">{tab === "backgrounds" ? "No backgrounds registered yet." : "No character cutouts registered yet."}</p>}
      </div>
    </section>
  );
};

function StatusMark({ status }: { status: string }) {
  const glyph = status === "incomplete" ? "●" : status === "empty" ? "○" : "✓";
  return <span className={`sb-status sb-status-${status}`}>{glyph}</span>;
}

export const StoryboardView: React.FC<Props> = ({ projectPath, storyboard, onRefresh, onToast }) => {
  const [pages, setPages] = useState<StoryboardPage[]>([]);
  const [check, setCheck] = useState<StoryboardData>(storyboard || {});
  const [pageIndex, setPageIndex] = useState(0);
  const [rail, setRail] = useState<string[]>([]);
  const [preview, setPreview] = useState("");
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const [zoom, setZoom] = useState(100);
  const request = useRef(0);
  const liveProject = useRef(projectPath);
  liveProject.current = projectPath;
  const folder = projectPath.replace(/[/\\][^/\\]+$/, "");
  const page = pages[Math.min(pageIndex, Math.max(0, pages.length - 1))];

  const load = async () => {
    const current = ++request.current;
    const project = projectPath;
    setError("");
    try {
      const data = await executeCommand<{ pages: StoryboardPage[] }>("storyboard.pages", { _expected_project: project });
      const completion = await executeCommand<StoryboardData>("storyboard.check", { _expected_project: project });
      const thumbs = await executeCommand<{ files: string[]; modified: number }>("storyboard.thumbnail", { rail: true, _expected_project: project });
      if (current !== request.current || liveProject.current !== project) return;
      setPages(data.pages);
      setCheck(completion);
      setRail(thumbs.files.map((file) => `${fileUrl(file)}?v=${thumbs.modified}`));
      setPageIndex((index) => Math.min(index, Math.max(0, data.pages.length - 1)));
    } catch (cause) {
      if (current === request.current) {
        setError(String(cause));
        setPreview("");
      }
    }
  };

  useEffect(() => {
    setPageIndex(0);
    void load();
    return () => {
      ++request.current;
    };
  }, [projectPath]);

  useEffect(() => {
    let cancelled = false;
    setPreview("");
    if (page) {
      void executeCommand<{ out: string; modified: number }>("storyboard.thumbnail", { page: page.page, scale: 0.7, _expected_project: projectPath })
        .then((shot) => {
          if (!cancelled) setPreview(`${fileUrl(shot.out)}?v=${shot.modified}`);
        })
        .catch((cause) => {
          if (!cancelled) setError(String(cause));
        });
    }
    return () => {
      cancelled = true;
    };
  }, [page, projectPath]);

  const action = async (name: string, run: () => Promise<unknown>, refresh = false) => {
    if (busy) return;
    setBusy(name);
    setError("");
    try {
      await run();
      if (refresh) {
        await load();
        await onRefresh();
      }
    } catch (cause) {
      setError(String(cause));
      onToast(String(cause));
    } finally {
      setBusy("");
    }
  };

  return (
    <div id="view-storyboard" className="storyboard-layout">
      <StageAssets onToast={onToast} />
      <aside className="sb-panel sb-index">
        <header>
          <h2>Storyboard &amp; design index</h2>
          <p>DesignCraft · {pages.length} pages</p>
        </header>
        <button id="storyboard-refresh" className="ghost" disabled={!!busy} onClick={() => void action("Refreshing…", async () => { await load(); await onRefresh(); })}>Sync &amp; refresh</button>
        <nav aria-label="Storyboard pages">
          {pages.map((item, index) => {
            const status = check.pages?.find((p) => p.page === item.page)?.status || "incomplete";
            return (
              <button
                id={`sb-nav-${item.page + 1}`}
                key={item.page}
                className={`sb-page-card ${page?.page === item.page ? "selected" : ""}`}
                aria-current={page?.page === item.page ? "page" : undefined}
                onClick={() => setPageIndex(index)}
              >
                <div className="sb-nav-preview">
                  {rail[item.page] ? <img src={rail[item.page]} alt={`Page ${item.page + 1}: ${item.title}`} /> : <span>{item.page + 1}</span>}
                </div>
                <span><StatusMark status={status} /> {item.page + 1}. {item.title}</span>
              </button>
            );
          })}
        </nav>
        <button id="storyboard-export" className="ghost" disabled={!!busy || !pages.length} onClick={() => void action("Exporting PDF & Deck…", async () => {
          await executeCommand("storyboard.export", { pdf: `${folder}/storyboard.pdf`, pptx: `${folder}/storyboard.pptx`, _expected_project: projectPath });
          onToast("Saved storyboard.pdf & storyboard.pptx");
        })}>Export PDF &amp; Deck</button>
      </aside>
      <main className="sb-panel sb-main">
        <header className="sb-toolbar">
          <button id="sb-prev" className="ghost" disabled={pageIndex === 0} onClick={() => setPageIndex((i) => i - 1)}>←</button>
          <strong>{page?.title ?? "DesignCraft storyboard"}</strong>
          <button id="sb-next" className="ghost" disabled={!pages.length || pageIndex >= pages.length - 1} onClick={() => setPageIndex((i) => i + 1)}>→</button>
          <label>Zoom <select id="sb-zoom" value={zoom} onChange={(event) => setZoom(Number(event.target.value))}>
            <option value={100}>Fit</option>
            <option value={150}>150%</option>
            <option value={200}>200%</option>
          </select></label>
          <button id="storyboard-open-editor" className="start" disabled={!!busy || !pages.length} onClick={() => void action("Opening DesignCraft…", () => executeCommand("storyboard.open", { _expected_project: projectPath }))}>Open in DesignCraft ↗</button>
        </header>
        <div id="sb-cards" className={`sb-spread ${zoom === 100 ? "fit" : "zoomed"}`}>
          {preview && page ? (
            <div className={`sb-spread-stage sb-zoom-${zoom}`}>
              <img src={preview} alt={page.title} />
            </div>
          ) : (
            <p>{pages.length ? "Rendering saved DesignCraft page…" : "No readable DesignCraft storyboard yet."}</p>
          )}
        </div>
        {error && <p role="alert" className="sb-error">{error}</p>}
        <footer role="status">{busy || "Edit directly in DesignCraft (Open in DesignCraft ↗) or via magic-cli storyboard commands · Static template chrome is locked."}</footer>
        {!pages.length && (
          <button id="storyboard-new" className="start" disabled={!!busy} onClick={() => void action("Creating storyboard…", () => executeCommand("storyboard.new", { path: `${folder}/storyboard.designcraft`, overwrite: false, _expected_project: projectPath }), true)}>
            Create from the DesignCraft template
          </button>
        )}
      </main>
    </div>
  );
};

export function PreservedStoryboard(props: Props & { active: boolean }) {
  const [opened, setOpened] = useState(props.active);
  useEffect(() => {
    if (props.active) setOpened(true);
  }, [props.active]);
  return opened || props.active ? (
    <div className="sb-preserved" hidden={!props.active}>
      <StoryboardView {...props} />
    </div>
  ) : null;
}
