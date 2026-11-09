import React, { useMemo, useState } from "react";
import type { ViewMode } from "../App";
import {
  CharacterSpriteSummary,
  CharacterSummary,
  CharacterViewSummary,
  executeCommand,
  fileUrl,
  pickFile,
  ProjectAsset,
  WorkshopData,
} from "../api";

interface StageAssetsProps {
  projectPath: string;
  view: ViewMode;
  data: WorkshopData | null;
  open: boolean;
  pinned: boolean;
  onToggleOpen: () => void;
  onTogglePin: () => void;
  selectedCharacterId?: string;
  onSelectCharacter: (charId: string, navigateToAnimation?: boolean) => void;
  onNavigate: (view: ViewMode, charId?: string) => void;
  onRefresh: () => Promise<void>;
  onToast: (message: string) => void;
}

interface UnifiedCard {
  id: string;
  charId?: string;
  assetId?: string;
  name: string;
  badge: string;
  thumb: string | null;
  role: "character" | "background";
  isReady: boolean;
  path?: string;
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

function getCharacterBadge(c: CharacterSummary): { label: string; ready: boolean } {
  const style = c.animation_style;
  const spriteCount = Array.isArray(c.sprites) ? c.sprites.length : 0;
  const has3D = Boolean(c.tripo_3d && (c.tripo_3d as { available?: boolean }).available);
  if (style === "rig_3d") {
    return { label: "3D Model · Anim ✓", ready: true };
  }
  if (style === "cutout_2_5d") {
    return {
      label: has3D
        ? `2.5D/3D Rig · ${spriteCount || 1} clips ✓`
        : spriteCount > 0
          ? `2.5D Rig · ${spriteCount} cycles ✓`
          : "2.5D Cutout Rig ✓",
      ready: true,
    };
  }
  if (style === "sprite_2d") {
    return { label: `2D Sprite · ${spriteCount} cycles ✓`, ready: true };
  }
  const hasRig =
    Array.isArray(c.views) &&
    c.views.some((v) => typeof v === "object" && v !== null && Boolean((v as CharacterViewSummary).ecproj));
  if (hasRig && spriteCount > 0) return { label: `2.5D Rig · ${spriteCount} cycles ✓`, ready: true };
  if (hasRig) return { label: "2.5D Rig ✓", ready: true };
  if (spriteCount > 0) return { label: `2D Sprite · ${spriteCount} cycles`, ready: true };
  if (has3D) {
    return { label: "3D Rig · GLB", ready: true };
  }
  return { label: "Cutout", ready: false };
}

/**
 * Universal persistent horizontal Asset Drawer mounted directly below `#app-bar` in `App.tsx`.
 *
 * Reads directly from `data` (`project.snapshot` / `session_snapshot` in Rust) so `Project`,
 * `Storyboard`, `Lyrics`, `Motion`, `Animation`, and `Studio` share one source of truth and
 * one place to add, inspect, or select project assets.
 */
export const StageAssets: React.FC<StageAssetsProps> = ({
  projectPath,
  view,
  data,
  open,
  pinned,
  onToggleOpen,
  onTogglePin,
  selectedCharacterId,
  onSelectCharacter,
  onNavigate,
  onRefresh,
  onToast,
}) => {
  const [tab, setTab] = useState<"characters" | "backgrounds">("characters");
  const [busy, setBusy] = useState<boolean>(false);

  const rawAssets: ProjectAsset[] = data?.assets?.assets ?? [];
  const rawCharacters: CharacterSummary[] = data?.characters ?? [];

  const { characterCards, backgroundCards } = useMemo(() => {
    const chars: UnifiedCard[] = [];
    const seenCharIds = new Set<string>();
    const seenAssetIds = new Set<string>();

    for (const c of rawCharacters) {
      const cid = c.id;
      seenCharIds.add(cid);
      const matchingAsset = rawAssets.find(
        (a) => a.id === cid || a.id.replace(/[^a-zA-Z0-9]+/g, "_") === cid || a.character === cid
      );
      if (matchingAsset) {
        seenAssetIds.add(matchingAsset.id);
      }
      const { label, ready } = getCharacterBadge(c);
      chars.push({
        id: matchingAsset?.id || cid,
        charId: cid,
        assetId: matchingAsset?.id,
        name: c.name || cid,
        badge: label,
        thumb: getCharacterThumb(c) || (matchingAsset?.path && /\.(png|jpe?g|webp|gif|svg)$/i.test(matchingAsset.path) ? matchingAsset.path : null),
        role: "character",
        isReady: ready,
        path: matchingAsset?.path,
      });
    }

    const bgs: UnifiedCard[] = [];
    for (const asset of rawAssets) {
      const isImg = Boolean(asset.path && /\.(png|jpe?g|webp|gif|svg)$/i.test(asset.path));
      if (asset.role === "background") {
        bgs.push({
          id: asset.id,
          assetId: asset.id,
          name: asset.name || asset.id,
          badge: "Background · Storyboard →",
          thumb: isImg ? asset.path! : null,
          role: "background",
          isReady: true,
          path: asset.path,
        });
      } else if (!seenAssetIds.has(asset.id)) {
        const normId = (asset.character || asset.id).replace(/[^a-zA-Z0-9]+/g, "_");
        seenCharIds.add(normId);
        seenAssetIds.add(asset.id);
        chars.push({
          id: asset.id,
          charId: normId,
          assetId: asset.id,
          name: asset.name || asset.id,
          badge: asset.kind === "model_3d" ? "3D Rig · Animate →" : "Cutout · Animate →",
          thumb: isImg ? asset.path! : null,
          role: "character",
          isReady: asset.kind === "model_3d",
          path: asset.path,
        });
      }
    }

    return { characterCards: chars, backgroundCards: bgs };
  }, [rawAssets, rawCharacters]);

  const totalCount = characterCards.length + backgroundCards.length;
  const visible = tab === "backgrounds" ? backgroundCards : characterCards;
  const activeCharId = selectedCharacterId || characterCards[0]?.charId || "";
  const activeCard = characterCards.find((c) => c.charId === activeCharId || c.id === activeCharId);

  const run = async (fn: () => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    try {
      await fn();
      await onRefresh();
    } catch (cause) {
      onToast(String(cause));
    } finally {
      setBusy(false);
    }
  };

  const handleAddAsset = () =>
    void run(async () => {
      const selected = await pickFile({
        title: tab === "backgrounds" ? "Add Background Plate" : "Add Character or Artwork Asset",
        extensions: ["png", "jpg", "jpeg", "webp", "gif", "svg", "ecproj", "psd", "pcraft", "glb", "gltf"],
      });
      if (typeof selected !== "string" || !selected) return;
      const name = (selected.split(/[/\\]/).pop() || selected).replace(/\.[^.]+$/, "");
      if (selected.toLowerCase().endsWith(".ecproj")) {
        const res = await executeCommand<{ character?: string }>("character.import", {
          path: selected,
          _expected_project: projectPath,
        });
        const cid = res.character || name.replace(/[^a-zA-Z0-9]+/g, "_");
        onSelectCharacter(cid, view === "animation");
        onToast(`Imported EffectCraft rig ${cid}`);
        return;
      }
      const slug = name.replace(/[^a-zA-Z0-9]+/g, "_");
      const destination =
        tab === "backgrounds"
          ? { background: name }
          : view === "animation"
            ? { character: slug }
            : { image: name };
      const added = await executeCommand<{ asset_id: string }>("project.add_asset", {
        path: selected,
        name,
        ...destination,
        load: false,
        _expected_project: projectPath,
      });
      if (tab === "characters") {
        onSelectCharacter(slug, false);
      }
      onToast(`Added ${added.asset_id}`);
    });

  const handleRemoveCard = (card: UnifiedCard) =>
    void run(async () => {
      const targetAssetId =
        card.assetId || rawAssets.find((a) => a.id === card.id || a.character === card.charId)?.id;
      if (!targetAssetId) {
        onToast(`${card.name} is a project folder asset`);
        return;
      }
      await executeCommand("project.remove_asset", {
        id: targetAssetId,
        _expected_project: projectPath,
      });
      onToast(`Removed ${card.name}`);
    });

  const handlePrepareArtwork = () => {
    if (view !== "animation") {
      onNavigate("animation", activeCharId || undefined);
    }
    window.setTimeout(() => {
      const pane = document.getElementById("magic-parts-pane") as HTMLDetailsElement | null;
      if (pane) {
        pane.open = true;
        pane.scrollIntoView({ behavior: "smooth", block: "nearest" });
      }
    }, 60);
  };

  return (
    <section
      id="stage-assets-panel"
      className={`app-assets-drawer ${open ? "open" : "collapsed"}`}
      data-ui-config="motion-asset-browser"
      aria-label="Stage Assets & Animate"
    >
      <div id="motion-asset-browser" className="app-assets-bar">
        <button
          id="stage-assets-toggle"
          type="button"
          className="ghost app-assets-toggle"
          aria-expanded={open}
          onClick={onToggleOpen}
          title={open ? "Collapse horizontal asset drawer" : "Expand horizontal asset drawer"}
        >
          <span className="chev">{open ? "▴" : "▾"}</span>
          <strong>Assets</strong>
          <span id="motion-assets-meta" className="muted">
            ({totalCount})
          </span>
        </button>

        <button
          id="stage-assets-pin"
          type="button"
          className={`${pinned ? "start" : "ghost"} app-assets-pin`}
          aria-pressed={pinned}
          aria-label={pinned ? "Unpin Asset Drawer" : "Pin Asset Drawer open"}
          onClick={onTogglePin}
          title={pinned ? "Unpin Asset Drawer (collapse when switching views)" : "Pin Asset Drawer open across all views"}
        >
          📌
        </button>

        <div className="app-assets-tabs" role="tablist" aria-label="Asset categories">
          <button
            id="stage-tab-characters"
            type="button"
            className={tab === "characters" ? "start" : "ghost"}
            onClick={() => {
              setTab("characters");
              if (!open) onToggleOpen();
            }}
          >
            Characters / Rig ({characterCards.length})
          </button>
          <button
            id="stage-tab-plates"
            type="button"
            className={tab === "backgrounds" ? "start" : "ghost"}
            onClick={() => {
              setTab("backgrounds");
              if (!open) onToggleOpen();
            }}
          >
            Backgrounds ({backgroundCards.length})
          </button>
        </div>

        {open && characterCards.length > 0 && (
          <div className="app-assets-active-char">
            <span
              id="motion-status-dot"
              className={`dot ${activeCard?.isReady ? "ok" : "warn"}`}
              title={activeCard?.badge || "Active character"}
            />
            <select
              id="motion-object"
              aria-label="Character"
              value={activeCharId}
              onChange={(e) => onSelectCharacter(e.target.value, view === "animation")}
            >
              {characterCards.map((c) => (
                <option key={c.id} value={c.charId || c.id}>
                  {c.name}
                </option>
              ))}
            </select>
          </div>
        )}

        <div className="app-assets-actions">
          {open && (
            <button
              id="motion-rebuild-rig"
              type="button"
              className="ghost"
              title="Mark parts, cut layers, or rebuild rig"
              onClick={handlePrepareArtwork}
            >
              ✂ Prepare artwork…
            </button>
          )}
          <button
            id="stage-add-asset"
            type="button"
            className="start"
            disabled={busy}
            onClick={handleAddAsset}
          >
            + Add Asset
          </button>
        </div>
      </div>

      <div id="motion-assets" className="app-assets-shelf" hidden={!open}>
        <div id="magic-assets-list" className="app-assets-row">
          {visible.map((card) => {
            const isSelected =
              card.role === "character" &&
              (card.charId === activeCharId || card.id === activeCharId);
            return (
              <div
                key={card.id}
                id={`stage-asset-${card.id}`}
                className={`app-asset-card ${isSelected ? "active" : ""}`}
                role="button"
                tabIndex={0}
                title={`${card.name} · ${card.badge}`}
                onClick={() => {
                  if (card.role === "character") {
                    void executeCommand("character.open", { id: card.assetId || card.charId || card.id }).catch(() => {});
                    onSelectCharacter(card.charId || card.id, true);
                  } else {
                    onNavigate("storyboard");
                  }
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    if (card.role === "character") {
                      void executeCommand("character.open", { id: card.assetId || card.charId || card.id }).catch(() => {});
                      onSelectCharacter(card.charId || card.id, true);
                    } else {
                      onNavigate("storyboard");
                    }
                  }
                }}
              >
                {card.thumb ? (
                  <img className="app-asset-thumb" src={fileUrl(card.thumb)} alt={card.name} loading="lazy" />
                ) : (
                  <div className="muted app-asset-placeholder">
                    {card.badge.includes("3D") ? "3D" : card.role === "background" ? "BG" : "RIG"}
                  </div>
                )}
                <div className="app-asset-meta">
                  <div className="app-asset-name">{card.name}</div>
                  <div className={`app-asset-sub ${card.isReady ? "ready" : "muted"}`}>{card.badge}</div>
                </div>
                {card.assetId && (
                  <button
                    id={`stage-asset-remove-${card.assetId}`}
                    type="button"
                    className="ghost app-asset-remove"
                    aria-label={`Remove ${card.name}`}
                    title={`Remove ${card.name} from project`}
                    onClick={(event) => {
                      event.stopPropagation();
                      handleRemoveCard(card);
                    }}
                  >
                    ×
                  </button>
                )}
              </div>
            );
          })}
          {!visible.length && (
            <span className="muted app-assets-empty">
              {tab === "backgrounds"
                ? "No background plates registered yet · click + Add Asset or drop files."
                : "No character cutouts or rigs yet · click + Add Asset or drop files."}
            </span>
          )}
        </div>
      </div>
    </section>
  );
};
