import React, { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { executeCommand, fileUrl, pickFile, ProjectAsset, StoryboardData, StoryboardPage, StoryboardSlot } from "../api";
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
  const [previews, setPreviews] = useState<Record<number, string>>({});
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const [zoom, setZoom] = useState(100);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [editing, setEditing] = useState<string | null>(null);
  const request = useRef(0);
  const liveProject = useRef(projectPath);
  const cardsRef = useRef<HTMLDivElement | null>(null);
  const programmaticScrollUntil = useRef(0);
  liveProject.current = projectPath;
  const folder = projectPath.replace(/[/\\][^/\\]+$/, "");
  const page = pages[Math.min(pageIndex, Math.max(0, pages.length - 1))];
  const slotKey = (pageNum: number, slot: StoryboardSlot) => `${pageNum}:${slot.label}`;

  const load = async () => {
    const current = ++request.current;
    const project = projectPath;
    setError("");
    try {
      const data = await executeCommand<{ pages: StoryboardPage[] }>("storyboard.pages", { full: true, _expected_project: project });
      const completion = await executeCommand<StoryboardData>("storyboard.check", { _expected_project: project });
      const thumbs = await executeCommand<{ files: string[]; modified: number }>("storyboard.thumbnail", { rail: true, scale: 0.45, _expected_project: project });
      if (current !== request.current || liveProject.current !== project) return;
      setPages(data.pages);
      setCheck(completion);
      setRail(thumbs.files.map((file) => `${fileUrl(file)}?v=${thumbs.modified}`));
      setPageIndex((index) => Math.min(index, Math.max(0, data.pages.length - 1)));
    } catch (cause) {
      if (current === request.current) {
        setError(String(cause));
      }
    }
  };

  useEffect(() => {
    setPageIndex(0);
    setDrafts({});
    setEditing(null);
    setPreviews({});
    void load();
    return () => {
      ++request.current;
    };
  }, [projectPath]);

  useEffect(() => {
    let cancelled = false;
    if (page) {
      const targetPage = page.page;
      void executeCommand<{ out: string; modified: number }>("storyboard.thumbnail", { page: targetPage, scale: 0.75, _expected_project: projectPath })
        .then((shot) => {
          if (!cancelled) {
            setPreviews((old) => ({ ...old, [targetPage]: `${fileUrl(shot.out)}?v=${shot.modified}` }));
          }
        })
        .catch((cause) => {
          if (!cancelled) setError(String(cause));
        });
    }
    return () => {
      cancelled = true;
    };
  }, [page, projectPath]);

  const scrollToPage = (index: number) => {
    const clamped = Math.max(0, Math.min(pages.length - 1, index));
    setPageIndex(clamped);
    setEditing(null);
    programmaticScrollUntil.current = Date.now() + 500;
    const target = document.getElementById(`sb-spread-${clamped}`);
    target?.scrollIntoView({ behavior: "smooth", block: "center" });
  };

  const handleScroll = () => {
    if (Date.now() < programmaticScrollUntil.current) return;
    const container = cardsRef.current;
    if (!container || !pages.length) return;
    const rect = container.getBoundingClientRect();
    const centerY = rect.top + rect.height / 2;
    let bestIndex = pageIndex;
    let bestDist = Infinity;
    for (let i = 0; i < pages.length; i++) {
      const el = document.getElementById(`sb-spread-${i}`);
      if (!el) continue;
      const r = el.getBoundingClientRect();
      const mid = r.top + r.height / 2;
      const dist = Math.abs(mid - centerY);
      if (dist < bestDist) {
        bestDist = dist;
        bestIndex = i;
      }
    }
    if (bestIndex !== pageIndex) {
      setPageIndex(bestIndex);
    }
  };

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

  const change = (pageNum: number, slot: StoryboardSlot, value: string) => {
    const key = slotKey(pageNum, slot);
    setDrafts((old) => {
      const next = { ...old };
      if (value === (slot.text ?? "")) delete next[key];
      else next[key] = value;
      return next;
    });
  };

  const cancel = (pageNum: number, slot: StoryboardSlot) => {
    const key = slotKey(pageNum, slot);
    setDrafts((old) => {
      const next = { ...old };
      delete next[key];
      return next;
    });
    setEditing(null);
  };

  const save = async (pageNum: number, slot: StoryboardSlot) => {
    const key = slotKey(pageNum, slot);
    const value = drafts[key];
    if (value === undefined) return;
    await action(
      "Saving field…",
      async () => {
        await executeCommand("storyboard.set", { page: pageNum, fields: { [slot.label]: value }, _expected_project: projectPath });
        setDrafts((old) => {
          const next = { ...old };
          delete next[key];
          return next;
        });
        setEditing(null);
      },
      true
    );
  };

  return (
    <div id="view-storyboard" className="storyboard-layout" data-document-dirty={Object.keys(drafts).length > 0} data-document-saving={busy === "Saving field…"}>
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
                onClick={() => scrollToPage(index)}
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
          <button id="sb-prev" className="ghost" disabled={pageIndex === 0} onClick={() => scrollToPage(pageIndex - 1)}>←</button>
          <strong>{page ? `${page.page + 1} / ${pages.length} · ${page.title}${page.section ? ` (${page.section})` : ""}` : "DesignCraft storyboard"}</strong>
          <button id="sb-next" className="ghost" disabled={!pages.length || pageIndex >= pages.length - 1} onClick={() => scrollToPage(pageIndex + 1)}>→</button>
          <label>Zoom <select id="sb-zoom" value={zoom} onChange={(event) => setZoom(Number(event.target.value))}>
            <option value={100}>Fit</option>
            <option value={150}>150%</option>
            <option value={200}>200%</option>
          </select></label>
          <button id="storyboard-open-editor" className="start" disabled={!!busy || !pages.length} onClick={() => void action("Opening DesignCraft…", () => executeCommand("storyboard.open", { _expected_project: projectPath }))}>Open in DesignCraft ↗</button>
        </header>
        <div id="sb-cards" ref={cardsRef} onScroll={handleScroll} className={`sb-spread ${zoom === 100 ? "fit" : "zoomed"}`}>
          {pages.length ? (
            pages.map((item) => {
              const imgSrc = previews[item.page] || rail[item.page];
              const [vx, vy, vx1, vy1] = item.bounds ?? [0, 0, 1600, 1000];
              const itemTextSlots = (item.slots || []).filter((slot) => slot.kind === "text" && !slot.locked && !slot.hidden);
              const isCurrent = page?.page === item.page;
              return (
                <section
                  id={`sb-spread-${item.page}`}
                  key={item.page}
                  className={`sb-page-viewport ${isCurrent ? "current" : ""}`}
                  aria-label={`Page ${item.page + 1}: ${item.title}`}
                >
                  <div className={`sb-spread-stage sb-zoom-${zoom}`}>
                    {imgSrc ? <img src={imgSrc} alt={item.title} /> : <div className="sb-spread-loading muted">Rendering page {item.page + 1}…</div>}
                    <svg className="sb-overlay" viewBox={`${vx} ${vy} ${vx1 - vx} ${vy1 - vy}`} preserveAspectRatio="xMidYMid meet">
                      {itemTextSlots.map((slot) => {
                        const b = slot.bounds;
                        if (!b || b[2] <= b[0] || b[3] <= b[1]) return null;
                        const key = slotKey(item.page, slot);
                        const active = editing === key;
                        const value = drafts[key] ?? slot.text ?? "";
                        const dirty = drafts[key] !== undefined;
                        return (
                          <g key={slot.label}>
                            <rect
                              id={isCurrent ? `sb-slot-${slot.label}` : `sb-slot-p${item.page}-${slot.label}`}
                              className={`sb-slot sb-slot-text ${active ? "active" : ""}`}
                              x={b[0]}
                              y={b[1]}
                              width={b[2] - b[0]}
                              height={b[3] - b[1]}
                              role="button"
                              tabIndex={0}
                              aria-label={`${slot.label}: click to edit text`}
                              onClick={() => {
                                setPageIndex(item.page);
                                setEditing(active ? null : key);
                              }}
                              onKeyDown={(event) => {
                                if (event.key === "Enter") {
                                  setPageIndex(item.page);
                                  setEditing(active ? null : key);
                                }
                              }}
                            >
                              <title>{slot.label}</title>
                            </rect>
                            {active && (() => {
                              const w = Math.min(Math.max(b[2] - b[0], 420), vx1 - vx);
                              const h = Math.min(Math.max(b[3] - b[1], 240), vy1 - vy);
                              const x = Math.max(vx, Math.min(b[0], vx1 - w));
                              const y = Math.max(vy, Math.min(b[1], vy1 - h));
                              return (
                                <foreignObject className="sb-canvas-editor" x={x} y={y} width={w} height={h}>
                                  <div className="sb-field-editor">
                                    <textarea
                                      id={`sb-canvas-field-${slot.label}`}
                                      aria-label={slot.label}
                                      rows={4}
                                      value={value}
                                      onChange={(event) => change(item.page, slot, event.target.value)}
                                    />
                                    <div>
                                      <button id={`sb-canvas-save-${slot.label}`} className="start" disabled={!!busy || !dirty} onClick={() => void save(item.page, slot)}>Save field</button>
                                      <button id={`sb-canvas-cancel-${slot.label}`} className="ghost" disabled={!!busy} onClick={() => cancel(item.page, slot)}>Cancel</button>
                                    </div>
                                  </div>
                                </foreignObject>
                              );
                            })()}
                          </g>
                        );
                      })}
                    </svg>
                  </div>
                </section>
              );
            })
          ) : (
            <p>{pages.length ? "Rendering saved DesignCraft page…" : "No readable DesignCraft storyboard yet."}</p>
          )}
        </div>
        {error && <p role="alert" className="sb-error">{error}</p>}
        <footer role="status">{busy || `${Object.keys(drafts).length ? `${Object.keys(drafts).length} unsaved field(s) · ` : ""}Click any text frame on the page to edit inline · Static template chrome is locked · Open in DesignCraft ↗ for layout edits.`}</footer>
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
