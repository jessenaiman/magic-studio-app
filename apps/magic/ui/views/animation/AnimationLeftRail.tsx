import React from "react";
import { CharacterSpriteSummary, CharacterSummary, CharacterViewSummary, fileUrl } from "../../api";

interface AnimationLeftRailProps {
  selectedCharId: string;
  assetsCount: number;
  characters: CharacterSummary[];
  onSelectCharacter: (charId: string) => void;
  onImportAsset: () => void;
  onRemoveAsset: (charId: string) => void;
  onPrepareArtwork: () => void;
}

function getCharacterThumb(c: CharacterSummary): string | null {
  if (c.image) return c.image;
  if (Array.isArray(c.views)) {
    for (const v of c.views) {
      if (typeof v === "object" && v !== null && "drawing" in v && (v as CharacterViewSummary).drawing) {
        return (v as CharacterViewSummary).drawing || null;
      }
    }
  }
  if (Array.isArray(c.sprites)) {
    for (const s of c.sprites) {
      if (typeof s === "object" && s !== null) {
        const sp = s as CharacterSpriteSummary;
        if (sp.frames && sp.frames.length > 0) return sp.frames[0];
        if (sp.preview) return sp.preview;
      }
    }
  }
  return null;
}

function getCharacterBadge(c: CharacterSummary): string {
  const hasRig =
    Array.isArray(c.views) &&
    c.views.some((v) => typeof v === "object" && v !== null && Boolean((v as CharacterViewSummary).ecproj));
  const spriteCount = Array.isArray(c.sprites) ? c.sprites.length : 0;
  if (hasRig && spriteCount > 0) return `Rig · ${spriteCount} cycles`;
  if (hasRig) return "Rigged";
  if (spriteCount > 0) return `${spriteCount} cycles`;
  if (c.tripo_3d && (c.tripo_3d as { available?: boolean }).available) return "3D model";
  return "Cutout";
}

export const AnimationLeftRail: React.FC<AnimationLeftRailProps> = ({
  selectedCharId,
  assetsCount,
  characters,
  onSelectCharacter,
  onImportAsset,
  onRemoveAsset,
  onPrepareArtwork,
}) => {
  const activeChar = characters.find((c) => c.id === selectedCharId);
  const isReady = Boolean(
    activeChar &&
      ((Array.isArray(activeChar.sprites) && activeChar.sprites.length > 0) ||
        (Array.isArray(activeChar.views) &&
          activeChar.views.some(
            (v) => typeof v === "object" && v !== null && Boolean((v as CharacterViewSummary).ecproj)
          )))
  );

  return (
    <aside
      id="motion-asset-browser"
      className="panel"
      data-look="assets"
      data-ui-config="motion-asset-browser"
      aria-label="Animation assets"
    >
      <div className="panel-head inline" data-look="bar">
        <span id="motion-status-dot" className={`dot ${isReady ? "ok" : "warn"}`} />
        <select
          id="motion-object"
          aria-label="Character"
          value={selectedCharId}
          onChange={(e) => onSelectCharacter(e.target.value)}
        >
          {characters.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name || c.id}
            </option>
          ))}
        </select>
        <button
          id="motion-import-asset"
          type="button"
          className="icon"
          title={`Import Asset (${assetsCount} in project)`}
          aria-label="Import character or artwork"
          onClick={onImportAsset}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M12 5v14M5 12h14" />
          </svg>
        </button>
        <button
          id="motion-remove-asset"
          type="button"
          className="icon"
          title="Remove asset from project"
          aria-label="Remove asset from project"
          onClick={() => onRemoveAsset(selectedCharId)}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
            <path d="M3 6h18" />
            <path d="M8 6V4h8v2" />
            <path d="M19 6l-1 14H6L5 6" />
          </svg>
        </button>
      </div>

      <div className="companion-artwork-home" style={{ padding: "var(--size-6) var(--size-8) 0" }}>
        <button
          id="motion-rebuild-rig"
          type="button"
          className="ghost"
          title="Mark parts, cut layers, or rebuild rig"
          onClick={onPrepareArtwork}
        >
          Prepare artwork…
        </button>
      </div>

      <details
        id="motion-assets"
        className="panel motion-section"
        data-look="assets"
        open
        style={{ margin: "var(--size-6) var(--size-8)", flex: 1, minHeight: 0, overflowY: "auto" }}
      >
        <summary className="panel-head" data-look="section">
          <span className="ico" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
              <path d="m12 2 9 5-9 5-9-5 9-5z" />
              <path d="m3 12 9 5 9-5" />
              <path d="m3 17 9 5 9-5" />
            </svg>
          </span>
          <span className="title">Assets</span>
          <span id="motion-assets-meta" className="meta">
            {characters.length}
          </span>
          <span className="chev" aria-hidden="true">
            ▸
          </span>
        </summary>
        <div className="motion-section-body">
          <div id="magic-assets-list" className="magic-assets-grid">
            {characters.map((c) => {
              const active = c.id === selectedCharId;
              const thumb = getCharacterThumb(c);
              const badge = getCharacterBadge(c);
              return (
                <div
                  key={c.id}
                  id={`char-select-${c.id}`}
                  role="button"
                  tabIndex={0}
                  className={`magic-asset-tile ${active ? "active" : ""}`}
                  onClick={() => onSelectCharacter(c.id)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      onSelectCharacter(c.id);
                    }
                  }}
                >
                  {thumb ? (
                    <img src={fileUrl(thumb)} alt={c.name || c.id} loading="lazy" />
                  ) : (
                    <span className="magic-asset-placeholder">IMG</span>
                  )}
                  <span className="magic-asset-name">{c.name || c.id}</span>
                  <span className="magic-asset-id">{badge}</span>
                </div>
              );
            })}
          </div>
        </div>
      </details>
    </aside>
  );
};
