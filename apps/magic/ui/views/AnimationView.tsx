import React, { useEffect, useState } from "react";
import {
  CharacterMetaResponse,
  CharacterSummary,
  executeCommand,
  fileUrl,
  pickFile,
  ProjectAsset,
} from "../api";

interface AnimationViewProps {
  projectPath: string;
  assets: ProjectAsset[];
  onRefresh: () => Promise<void>;
  onToast: (msg: string) => void;
}

interface CharacterListResponse {
  characters?: CharacterSummary[];
}

export const AnimationView: React.FC<AnimationViewProps> = ({
  projectPath,
  assets,
  onRefresh,
  onToast,
}) => {
  const [characters, setCharacters] = useState<CharacterSummary[]>([]);
  const [selectedCharId, setSelectedCharId] = useState<string>("");
  const [selectedView, setSelectedView] = useState<string>("front");
  const [meta, setMeta] = useState<CharacterMetaResponse | null>(null);
  const [framePath, setFramePath] = useState<string>("");
  const [selectedPart, setSelectedPart] = useState<string>("root");
  const [propName, setPropName] = useState<string>("rotation");
  const [propValue, setPropValue] = useState<number>(0);
  const [propTime, setPropTime] = useState<number>(0);
  const [actionName, setActionName] = useState<string>("wave");
  const [shotDuration, setShotDuration] = useState<number>(4);
  const [shotScale, setShotScale] = useState<number>(100);
  const [placeTime, setPlaceTime] = useState<number>(0);
  const [placeTrack, setPlaceTrack] = useState<string>("V2");
  const [lastRenderedClip, setLastRenderedClip] = useState<string>("");
  const [inspectJson, setInspectJson] = useState<string>("");
  const [busy, setBusy] = useState<boolean>(false);

  const loadCharacters = async () => {
    try {
      const res = await executeCommand<CharacterListResponse>("character.list");
      const list = res.characters || [];
      setCharacters(list);
      if (list.length > 0) {
        const target =
          list.find((c) => c.id === selectedCharId) || list[0];
        await selectCharacter(target.id, target.views?.[0] || "front");
      }
    } catch (err) {
      onToast(String(err));
    }
  };

  const selectCharacter = async (charId: string, view = "front") => {
    setSelectedCharId(charId);
    setSelectedView(view);
    try {
      const m = await executeCommand<CharacterMetaResponse>("character.meta", {
        character: charId,
      });
      setMeta(m);
      const parts =
        m.views?.find((v) => v.view === view)?.parts || m.parts || [];
      if (parts.length > 0) {
        setSelectedPart(parts[0].id);
      }
      if (m.shots && m.shots.length > 0) {
        setLastRenderedClip(m.shots[m.shots.length - 1].path);
      }
      await refreshFrame(charId, view, propTime);
    } catch {
      setMeta(null);
    }
  };

  const refreshFrame = async (charId: string, view: string, time: number) => {
    if (!charId) return;
    try {
      const res = await executeCommand<{ frame?: string; path?: string }>(
        "character.frame",
        {
          character: charId,
          view,
          time,
          max_side: 900,
        },
      );
      const p = res.frame || res.path || "";
      if (p) setFramePath(`${p}?t=${Date.now()}`);
    } catch {
      // character may not have a rigged .ecproj yet
    }
  };

  useEffect(() => {
    void loadCharacters();
  }, [projectPath]);

  const handleImportAsset = async () => {
    const selected = await pickFile({
      title: "Import Character / Artwork Asset",
      extensions: ["png", "webp", "jpg", "jpeg", "psd", "pcraft", "svg", "glb"],
    });
    if (typeof selected !== "string" || !selected) return;
    setBusy(true);
    try {
      const name =
        selected
          .split("/")
          .pop()
          ?.replace(/\.[^.]+$/, "") || "character";
      const charSlug = name.toLowerCase().replace(/[^a-z0-9]+/g, "-");
      await executeCommand("project.add_asset", {
        path: selected,
        character: charSlug,
        name,
        kind: "image",
      });
      await onRefresh();
      await loadCharacters();
      onToast(`Imported asset ${name}`);
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleKeyTransform = async () => {
    if (!selectedCharId) return;
    setBusy(true);
    try {
      await executeCommand("character.set", {
        character: selectedCharId,
        view: selectedView,
        part: selectedPart,
        prop: propName,
        value: propValue,
        time: propTime,
      });
      await refreshFrame(selectedCharId, selectedView, propTime);
      onToast(
        `Keyed ${selectedPart}.${propName} = ${propValue} at ${propTime}s`,
      );
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleKeyAction = async () => {
    if (!selectedCharId) return;
    setBusy(true);
    try {
      await executeCommand("character.action", {
        character: selectedCharId,
        view: selectedView,
        action: actionName,
        at: propTime,
        times: 1,
      });
      await refreshFrame(selectedCharId, selectedView, propTime);
      onToast(`Keyed action '${actionName}' on ${selectedCharId}`);
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleRenderClip = async () => {
    if (!selectedCharId) return;
    setBusy(true);
    try {
      const res = await executeCommand<{
        clip?: string;
        path?: string;
        output?: string;
      }>("character.render", {
        character: selectedCharId,
        view: selectedView,
        start: 0,
        end: shotDuration,
        fps: 24,
        format: "clip",
      });
      const clipPath = res.clip || res.path || res.output || "";
      if (clipPath) setLastRenderedClip(clipPath);
      await selectCharacter(selectedCharId, selectedView);
      onToast(`Rendered animation clip ${clipPath || selectedCharId}`);
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handlePlaceOnTimeline = async () => {
    if (!lastRenderedClip) {
      onToast("Render or select an animation clip (.mov/.mp4) first");
      return;
    }
    setBusy(true);
    try {
      await executeCommand("video.place_clip", {
        clip: lastRenderedClip,
        from: placeTime,
        track: placeTrack,
      });
      onToast(
        `Placed ${lastRenderedClip.split("/").pop()} on FilmCraft track ${placeTrack} at ${placeTime}s`,
      );
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handlePhotoCraftFlip = async (assetPath: string) => {
    setBusy(true);
    try {
      await executeCommand("photo.exec", {
        path: assetPath,
        id: "image.imageRotation.flipCanvasHorizontal",
        save: assetPath,
      });
      await onRefresh();
      onToast("Flipped image horizontally via PhotoCraft engine");
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handlePhotoCraftInspect = async (assetPath: string) => {
    try {
      const res = await executeCommand("photo.exec", {
        path: assetPath,
        id: "document.inspect",
      });
      setInspectJson(JSON.stringify(res, null, 2));
      onToast("Inspected layers via PhotoCraft engine");
    } catch (err) {
      onToast(String(err));
    }
  };

  const handleVectorCraftInspect = async (assetPath: string) => {
    try {
      const res = await executeCommand("vector.exec", {
        path: assetPath,
        mode: "info",
      });
      setInspectJson(JSON.stringify(res, null, 2));
      onToast("Inspected asset via VectorCraft");
    } catch (err) {
      onToast(String(err));
    }
  };

  const currentParts =
    meta?.views?.find((v) => v.view === selectedView)?.parts ||
    meta?.parts ||
    [];

  return (
    <div
      id="view-animation"
      style={{
        display: "grid",
        gridTemplateColumns: "280px 1fr 320px",
        gap: "12px",
        height: "100%",
        padding: "12px",
      }}
    >
      {/* Left Rail: Characters & Project Assets */}
      <aside
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
          <strong>Characters & Assets</strong>
          <button
            id="anim-import-asset"
            className="start"
            disabled={busy}
            onClick={handleImportAsset}
          >
            + Add Asset
          </button>
        </div>

        {characters.length > 0 && (
          <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
            <span className="muted" style={{ fontSize: "11px" }}>
              Rigged / Character Puppets ({characters.length})
            </span>
            {characters.map((c) => (
              <button
                key={c.id}
                id={`char-select-${c.id}`}
                className={c.id === selectedCharId ? "start" : "ghost"}
                style={{ textAlign: "left", justifyContent: "space-between" }}
                onClick={() =>
                  void selectCharacter(c.id, c.views?.[0] || "front")
                }
              >
                <span>{c.name || c.id}</span>
                <span className="muted" style={{ fontSize: "11px" }}>
                  {(c.views || ["front"]).join(", ")}
                </span>
              </button>
            ))}
          </div>
        )}

        {assets.length > 0 && (
          <div
            style={{
              display: "flex",
              flexDirection: "column",
              gap: "6px",
              marginTop: "8px",
            }}
          >
            <span className="muted" style={{ fontSize: "11px" }}>
              Project Artwork ({assets.length})
            </span>
            {assets.map((asset) => (
              <div
                key={asset.id}
                style={{
                  background: "var(--surface-1, #12151a)",
                  padding: "8px",
                  borderRadius: "6px",
                  display: "flex",
                  flexDirection: "column",
                  gap: "6px",
                }}
              >
                <strong style={{ fontSize: "12px" }}>{asset.name}</strong>
                {asset.path && (
                  <div style={{ display: "flex", gap: "4px", flexWrap: "wrap" }}>
                    <button
                      className="ghost"
                      style={{ fontSize: "11px", padding: "2px 6px" }}
                      onClick={() => void handlePhotoCraftInspect(asset.path!)}
                    >
                      PhotoCraft Info
                    </button>
                    <button
                      className="ghost"
                      style={{ fontSize: "11px", padding: "2px 6px" }}
                      onClick={() => void handlePhotoCraftFlip(asset.path!)}
                    >
                      Flip H
                    </button>
                    <button
                      className="ghost"
                      style={{ fontSize: "11px", padding: "2px 6px" }}
                      onClick={() => void handleVectorCraftInspect(asset.path!)}
                    >
                      VectorCraft Info
                    </button>
                    <button
                      className="ghost"
                      style={{ fontSize: "11px", padding: "2px 6px" }}
                      onClick={() =>
                        void executeCommand("workspace.open", {
                          path: asset.path,
                        })
                      }
                    >
                      Open Editor
                    </button>
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </aside>

      {/* Center Stage: Live EffectCraft Frame Preview & Scrubber */}
      <main
        style={{
          background: "var(--surface-1, #12151a)",
          borderRadius: "8px",
          padding: "16px",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "space-between",
          gap: "12px",
          overflow: "auto",
        }}
      >
        <div
          style={{
            width: "100%",
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
          }}
        >
          <strong>
            Animation Stage{" "}
            {selectedCharId ? `· ${selectedCharId} (${selectedView})` : ""}
          </strong>
          <div style={{ display: "flex", gap: "6px" }}>
            {selectedCharId && (
              <>
                <button
                  id="anim-open-effectcraft"
                  className="ghost"
                  onClick={() =>
                    void executeCommand("character.open", {
                      character: selectedCharId,
                      view: selectedView,
                      app: "effectcraft",
                    })
                  }
                >
                  Open in EffectCraft
                </button>
                <button
                  id="anim-open-photocraft"
                  className="ghost"
                  onClick={() =>
                    void executeCommand("character.open", {
                      character: selectedCharId,
                      view: selectedView,
                      app: "photocraft",
                    })
                  }
                >
                  Open in PhotoCraft
                </button>
              </>
            )}
          </div>
        </div>

        <div
          id="anim-stage-preview"
          style={{
            width: "100%",
            maxWidth: "760px",
            aspectRatio: "16 / 9",
            background: "#0d1016",
            border: "1px solid var(--border, #2a313c)",
            borderRadius: "8px",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            overflow: "hidden",
          }}
        >
          {framePath ? (
            <img
              src={fileUrl(framePath.split("?")[0])}
              alt={selectedCharId}
              style={{
                maxWidth: "100%",
                maxHeight: "100%",
                objectFit: "contain",
              }}
            />
          ) : (
            <span className="muted">
              {selectedCharId
                ? "Click 'Refresh Frame' or key an action to preview EffectCraft stage"
                : "Select or import a character asset on the left"}
            </span>
          )}
        </div>

        {/* Time Scrubber */}
        <div
          style={{
            width: "100%",
            display: "flex",
            alignItems: "center",
            gap: "10px",
          }}
        >
          <span className="muted" style={{ fontSize: "12px" }}>
            Time ({propTime.toFixed(2)}s)
          </span>
          <input
            id="anim-time-scrub"
            type="range"
            min={0}
            max={shotDuration}
            step={0.1}
            value={propTime}
            onChange={(e) => {
              const t = Number(e.target.value);
              setPropTime(t);
              void refreshFrame(selectedCharId, selectedView, t);
            }}
            style={{ flex: 1 }}
          />
          <button
            className="ghost"
            onClick={() => void refreshFrame(selectedCharId, selectedView, propTime)}
          >
            Refresh Frame
          </button>
        </div>

        {inspectJson && (
          <pre
            style={{
              width: "100%",
              maxHeight: "160px",
              overflow: "auto",
              background: "#0b0d11",
              padding: "10px",
              borderRadius: "6px",
              fontSize: "11px",
            }}
          >
            {inspectJson}
          </pre>
        )}
      </main>

      {/* Right Inspector: Keyframe Transforms, Actions & FilmCraft Handoff */}
      <aside
        id="anim-inspector"
        style={{
          background: "var(--surface-2, #181c22)",
          borderRadius: "8px",
          padding: "12px",
          display: "flex",
          flexDirection: "column",
          gap: "12px",
          overflowY: "auto",
        }}
      >
        <strong>Motion & Layer Inspector</strong>

        <label style={{ display: "grid", gap: "4px", fontSize: "12px" }}>
          <span>Part / Layer</span>
          <select
            id="anim-part-select"
            value={selectedPart}
            onChange={(e) => setSelectedPart(e.target.value)}
          >
            {currentParts.length > 0 ? (
              currentParts.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name || p.id}
                </option>
              ))
            ) : (
              <option value="root">root</option>
            )}
          </select>
        </label>

        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "8px" }}>
          <label style={{ display: "grid", gap: "4px", fontSize: "12px" }}>
            <span>Property</span>
            <select
              id="anim-prop-name"
              value={propName}
              onChange={(e) => setPropName(e.target.value)}
            >
              <option value="rotation">rotation</option>
              <option value="scale">scale</option>
              <option value="opacity">opacity</option>
              <option value="position">position</option>
            </select>
          </label>
          <label style={{ display: "grid", gap: "4px", fontSize: "12px" }}>
            <span>Value</span>
            <input
              id="anim-prop-value"
              type="number"
              value={propValue}
              onChange={(e) => setPropValue(Number(e.target.value))}
            />
          </label>
        </div>

        <button
          id="anim-key-transform"
          className="ghost"
          disabled={busy || !selectedCharId}
          onClick={handleKeyTransform}
        >
          Key Transform at {propTime.toFixed(1)}s
        </button>

        <hr style={{ borderColor: "var(--border, #2a313c)", width: "100%" }} />

        <label style={{ display: "grid", gap: "4px", fontSize: "12px" }}>
          <span>Pose Action Cycle</span>
          <div style={{ display: "flex", gap: "6px" }}>
            <select
              id="anim-action-select"
              value={actionName}
              onChange={(e) => setActionName(e.target.value)}
              style={{ flex: 1 }}
            >
              <option value="idle">idle</option>
              <option value="wave">wave</option>
              <option value="walk_down">walk_down</option>
              <option value="walk_left">walk_left</option>
              <option value="walk_right">walk_right</option>
              <option value="jump">jump</option>
              <option value="turn">turn</option>
            </select>
            <button
              id="anim-key-action"
              className="ghost"
              disabled={busy || !selectedCharId}
              onClick={handleKeyAction}
            >
              Key Action
            </button>
          </div>
        </label>

        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "8px" }}>
          <label style={{ display: "grid", gap: "4px", fontSize: "12px" }}>
            <span>Duration (s)</span>
            <input
              id="anim-shot-duration"
              type="number"
              min={1}
              max={60}
              value={shotDuration}
              onChange={(e) => setShotDuration(Number(e.target.value))}
            />
          </label>
          <label style={{ display: "grid", gap: "4px", fontSize: "12px" }}>
            <span>Scale (%)</span>
            <input
              id="anim-shot-scale"
              type="number"
              min={10}
              max={400}
              value={shotScale}
              onChange={(e) => setShotScale(Number(e.target.value))}
            />
          </label>
        </div>

        <button
          id="anim-render-clip"
          className="start"
          disabled={busy || !selectedCharId}
          onClick={handleRenderClip}
        >
          {busy ? "Working…" : "Render Animation Clip (.mov)"}
        </button>

        <hr style={{ borderColor: "var(--border, #2a313c)", width: "100%" }} />

        <strong>Handoff to FilmCraft Timeline</strong>
        <label style={{ display: "grid", gap: "4px", fontSize: "12px" }}>
          <span>Rendered Clip Path</span>
          <input
            id="anim-clip-path"
            value={lastRenderedClip}
            onChange={(e) => setLastRenderedClip(e.target.value)}
            placeholder="/path/to/rendered.mov"
          />
        </label>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "8px" }}>
          <label style={{ display: "grid", gap: "4px", fontSize: "12px" }}>
            <span>Track</span>
            <select
              id="anim-place-track"
              value={placeTrack}
              onChange={(e) => setPlaceTrack(e.target.value)}
            >
              <option value="V2">V2 (Characters)</option>
              <option value="V1">V1 (Backgrounds)</option>
              <option value="V3">V3 (Overlays)</option>
            </select>
          </label>
          <label style={{ display: "grid", gap: "4px", fontSize: "12px" }}>
            <span>At Seconds</span>
            <input
              id="anim-place-from"
              type="number"
              min={0}
              step={0.5}
              value={placeTime}
              onChange={(e) => setPlaceTime(Number(e.target.value))}
            />
          </label>
        </div>
        <button
          id="anim-place-clip-btn"
          className="start"
          disabled={busy || !lastRenderedClip}
          onClick={handlePlaceOnTimeline}
        >
          Place Clip on Film Timeline
        </button>
      </aside>
    </div>
  );
};
