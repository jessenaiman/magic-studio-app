import React, { useEffect, useRef, useState } from "react";
import { executeCommand, fileUrl, StoryboardData, StoryboardPage } from "../api";
import "../storyboard.css";

interface Props {
  projectPath: string;
  storyboard?: StoryboardData;
  onRefresh: () => Promise<void>;
  onToast: (message: string) => void;
}

/** A storyboard page is a spread of Letter pages; older documents without `document_pages` are one page each. */
const docPages = (item: StoryboardPage) => item.document_pages?.length ? item.document_pages : [item.page];
const LETTER_W = 612;

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
  const request = useRef(0);
  const liveProject = useRef(projectPath);
  const cardsRef = useRef<HTMLDivElement | null>(null);
  const programmaticScrollUntil = useRef(0);
  liveProject.current = projectPath;
  const folder = projectPath.replace(/[/\\][^/\\]+$/, "");
  const page = pages[Math.min(pageIndex, Math.max(0, pages.length - 1))];

  const load = async () => {
    const current = ++request.current;
    const project = projectPath;
    setError("");
    try {
      const data = await executeCommand<{ pages: StoryboardPage[] }>("storyboard.pages", { full: true, _expected_project: project });
      const completion = await executeCommand<StoryboardData>("storyboard.check", { _expected_project: project });
      const thumbs = await executeCommand<{ files: string[]; modified: number }>("storyboard.thumbnail", { rail: true, scale: 0.45, _expected_project: project });
      if (current !== request.current || liveProject.current !== project) return;
      // A view switch can catch DesignCraft mid-write and return an empty page list; keep the
      // pages already on screen instead of flashing "No readable storyboard".
      setPages((old) => (data.pages.length ? data.pages : old));
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
    setPreviews({});
    void load();
    return () => {
      ++request.current;
    };
  }, [projectPath]);

  useEffect(() => {
    let cancelled = false;
    if (page) {
      for (const docPage of docPages(page)) {
        void executeCommand<{ out: string; modified: number }>("storyboard.thumbnail", { document_page: docPage, scale: 0.75, _expected_project: projectPath })
          .then((shot) => {
            if (!cancelled) {
              setPreviews((old) => ({ ...old, [docPage]: `${fileUrl(shot.out)}?v=${shot.modified}` }));
            }
          })
          .catch((cause) => {
            if (!cancelled) setError(String(cause));
          });
      }
    }
    return () => {
      cancelled = true;
    };
  }, [page, projectPath]);

  const scrollToPage = (index: number) => {
    const clamped = Math.max(0, Math.min(pages.length - 1, index));
    setPageIndex(clamped);
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

  return (
    <div id="view-storyboard" className="storyboard-layout">
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
                  {docPages(item).map((n) => (rail[n] ? <img key={n} src={rail[n]} alt={`Page ${n + 1}: ${item.title}`} /> : <span key={n}>{n + 1}</span>))}
                </div>
                <span><StatusMark status={status} /> {item.page + 1}. {item.title}</span>
              </button>
            );
          })}
        </nav>
        <button id="storyboard-export" className="ghost" disabled={!!busy || !pages.length} onClick={() => void action("Exporting print, screen & pages…", async () => {
          await executeCommand("storyboard.export", { dir: `${folder}/export`, _expected_project: projectPath });
          onToast("Exported print PDF, screen PDF & page PNGs to export/");
        })}>Export</button>
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
              const [, vy, , vy1] = item.bounds ?? [0, 0, LETTER_W, 792];
              const itemTextSlots = (item.slots || []).filter((slot) => slot.kind === "text" && !slot.locked && !slot.hidden);
              const isCurrent = page?.page === item.page;
              return (
                <section
                  id={`sb-spread-${item.page}`}
                  key={item.page}
                  className={`sb-page-viewport ${isCurrent ? "current" : ""}`}
                  aria-label={`Page ${item.page + 1}: ${item.title}`}
                >
                  {docPages(item).map((docPage, side) => {
                  const imgSrc = previews[docPage] || rail[docPage];
                  const left = side * LETTER_W;
                  const pageSlots = itemTextSlots.filter((slot) => slot.bounds && slot.bounds[0] >= left && slot.bounds[0] < left + LETTER_W);
                  return (
                  <div key={docPage} id={`sb-page-${docPage}`} className={`sb-spread-stage sb-zoom-${zoom}`}>
                    {imgSrc ? <img src={imgSrc} alt={`${item.title} · page ${docPage + 1}`} /> : <div className="sb-spread-loading muted">Rendering page {docPage + 1}…</div>}
                    <svg className="sb-overlay" viewBox={`${left} ${vy} ${LETTER_W} ${vy1 - vy}`} preserveAspectRatio="xMidYMid meet">
                      {pageSlots.map((slot) => {
                        const b = slot.bounds;
                        if (!b || b[2] <= b[0] || b[3] <= b[1]) return null;
                        return (
                          <g key={slot.label}>
                            <rect
                              id={isCurrent ? `sb-slot-${slot.label}` : `sb-slot-p${item.page}-${slot.label}`}
                              className="sb-slot sb-slot-text"
                              x={b[0]}
                              y={b[1]}
                              width={b[2] - b[0]}
                              height={b[3] - b[1]}
                              role="button"
                              tabIndex={0}
                              aria-label={`${slot.label}: select page`}
                              onClick={() => setPageIndex(item.page)}
                              onKeyDown={(event) => {
                                if (event.key === "Enter") setPageIndex(item.page);
                              }}
                            >
                              <title>{slot.label}</title>
                            </rect>
                          </g>
                        );
                      })}
                    </svg>
                  </div>
                  );
                  })}
                </section>
              );
            })
          ) : (
            <p>{pages.length ? "Rendering saved DesignCraft page…" : "No readable DesignCraft storyboard yet."}</p>
          )}
        </div>
        {error && <p role="alert" className="sb-error">{error}</p>}
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
