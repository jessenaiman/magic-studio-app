import React, { useState } from "react";
import { executeCommand, fileUrl, StoryboardData } from "../api";

interface StoryboardViewProps {
  storyboard?: StoryboardData;
  onRefresh: () => Promise<void>;
  onToast: (msg: string) => void;
}

export const StoryboardView: React.FC<StoryboardViewProps> = ({
  storyboard,
  onRefresh,
  onToast,
}) => {
  const [pageIndex, setPageIndex] = useState(0);
  const [busy, setBusy] = useState(false);

  const pages = storyboard?.previews || [];
  const safePage = Math.max(0, Math.min(pageIndex, Math.max(0, pages.length - 1)));
  const pageTitle = (i: number) =>
    storyboard?.page_titles?.[i] || `Page ${i + 1}`;
  const cues = storyboard?.page_cues?.[safePage] || [];
  const [width, height] = storyboard?.page_size || [1080, 1600];

  const handleRenderPages = async () => {
    setBusy(true);
    try {
      await executeCommand("storyboard.render");
      await onRefresh();
      onToast("Storyboard pages rendered via DeckCraft");
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleOpenDeckCraft = async () => {
    if (!storyboard?.deck) return;
    try {
      await executeCommand("workspace.open", { path: storyboard.deck });
      onToast("Opened storyboard in DeckCraft");
    } catch (err) {
      onToast(String(err));
    }
  };

  return (
    <div
      id="view-storyboard"
      style={{
        display: "grid",
        gridTemplateColumns: "260px 1fr",
        gap: "16px",
        height: "100%",
        padding: "16px",
      }}
    >
      <aside
        id="storyboard-rail"
        style={{
          background: "var(--surface-2, #181c22)",
          borderRadius: "8px",
          padding: "12px",
          display: "flex",
          flexDirection: "column",
          gap: "10px",
          overflowY: "auto",
        }}
      >
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <strong>Storyboard (DeckCraft)</strong>
          <span id="sb-page" className="muted" style={{ fontSize: "12px" }}>
            {pages.length ? `${safePage + 1} / ${pages.length}` : "0 pages"}
          </span>
        </div>

        <div style={{ display: "flex", gap: "6px", flexWrap: "wrap" }}>
          <button
            id="sb-render"
            className="start"
            disabled={busy}
            onClick={handleRenderPages}
          >
            {busy ? "Rendering…" : "Render Pages"}
          </button>
          {storyboard?.deck && (
            <button
              id="storyboard-edit-deck"
              className="ghost"
              onClick={handleOpenDeckCraft}
            >
              Edit in DeckCraft
            </button>
          )}
        </div>

        {storyboard?.deck && (
          <span
            className="muted"
            style={{ fontSize: "11px", wordBreak: "break-all" }}
          >
            {storyboard.deck}
          </span>
        )}

        {pages.length > 0 && (
          <nav
            aria-label="Storyboard contents"
            style={{ display: "flex", flexDirection: "column", gap: "4px" }}
          >
            {pages.map((_, i) => (
              <button
                key={i}
                id={`sb-nav-${i + 1}`}
                className={safePage === i ? "start" : "ghost"}
                style={{ textAlign: "left", justifyContent: "flex-start" }}
                onClick={() => setPageIndex(i)}
              >
                {i + 1}. {pageTitle(i)}
              </button>
            ))}
          </nav>
        )}

        {cues.length > 0 && (
          <section
            aria-label="Selected scene lyrics"
            style={{
              marginTop: "8px",
              paddingTop: "8px",
              borderTop: "1px solid var(--border, #2a313c)",
            }}
          >
            <strong style={{ fontSize: "12px" }}>Scene Lyrics</strong>
            {cues.map((line, idx) => (
              <p key={idx} style={{ margin: "4px 0", fontSize: "12px" }}>
                {line.text}
              </p>
            ))}
          </section>
        )}
      </aside>

      <main
        id="sb-cards"
        style={{
          background: "var(--surface-1, #12151a)",
          borderRadius: "8px",
          padding: "16px",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          overflow: "auto",
        }}
      >
        {pages.length === 0 ? (
          <div className="panel-empty">
            <p>
              {storyboard?.error ||
                (storyboard?.deck
                  ? "Click 'Render Pages' to draw the DeckCraft storyboard pages."
                  : "No saved storyboard deck (.deckcraft) for this project yet.")}
            </p>
          </div>
        ) : (
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              gap: "12px",
              width: "100%",
            }}
          >
            <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
              <button
                id="sb-prev"
                className="ghost"
                disabled={safePage === 0}
                onClick={() => setPageIndex(safePage - 1)}
              >
                ← Prev
              </button>
              <strong>{pageTitle(safePage)}</strong>
              <button
                id="sb-next"
                className="ghost"
                disabled={safePage + 1 >= pages.length}
                onClick={() => setPageIndex(safePage + 1)}
              >
                Next →
              </button>
            </div>
            <div
              id={`sb-sheet-${safePage}`}
              style={{
                aspectRatio: `${width} / ${height}`,
                maxHeight: "74vh",
                borderRadius: "6px",
                overflow: "hidden",
                border: "1px solid var(--border, #2a313c)",
                background: "#000",
              }}
            >
              <img
                src={fileUrl(pages[safePage])}
                alt={pageTitle(safePage)}
                style={{
                  width: "100%",
                  height: "100%",
                  objectFit: "contain",
                  display: "block",
                }}
              />
            </div>
          </div>
        )}
      </main>
    </div>
  );
};
