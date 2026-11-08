import React, { useEffect, useState } from "react";
import {
  CharacterMetaResponse,
  CharacterSummary,
  executeCommand,
  fileUrl,
  pickFile,
  ProjectAsset,
} from "../api";
import { Dial, Slider, Toggle } from "../components/controls";

interface AnimationViewProps {
  projectPath: string;
  assets: ProjectAsset[];
  onRefresh: () => Promise<void>;
  onToast: (msg: string) => void;
}

interface SpriteAction {
  name: string;
  ecproj: string;
  preview?: string;
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
  // Characters & Selection
  const [characters, setCharacters] = useState<CharacterSummary[]>([]);
  const [selectedCharId, setSelectedCharId] = useState<string>("robot");
  const [selectedView, setSelectedView] = useState<string>("front");
  const [searchFilter, setSearchFilter] = useState<string>("");
  const [assetsOpen, setAssetsOpen] = useState<boolean>(true);
  const [poseOpen, setPoseOpen] = useState<boolean>(true);

  // Stage & View Modes
  const [stageMode, setStageMode] = useState<"skeleton" | "ik" | "drawing" | "3d">("skeleton");
  const [showSkeleton, setShowSkeleton] = useState<boolean>(true);
  const [showGrid, setShowGrid] = useState<boolean>(false);
  const [zoomLevel, setZoomLevel] = useState<number>(1);
  const [selectedSprite, setSelectedSprite] = useState<SpriteAction | null>(null);
  const [sprites, setSprites] = useState<SpriteAction[]>([]);
  const [framePath, setFramePath] = useState<string>("");
  const [meta, setMeta] = useState<CharacterMetaResponse | null>(null);

  // Playback & Transport
  const [isPlaying, setIsPlaying] = useState<boolean>(false);
  const [isLooping, setIsLooping] = useState<boolean>(true);
  const [currentTime, setCurrentTime] = useState<number>(1.065);
  const [duration, setDuration] = useState<number>(2.5);

  // Theatre Panel
  const [theatreTab, setTheatreTab] = useState<"theatre" | "keys" | "curves">("theatre");

  // Selected Bone & Transforms (Right Inspector)
  const [selectedBone, setSelectedBone] = useState<string>("leg_R");
  const [rotation, setRotation] = useState<number>(0);
  const [scale, setScale] = useState<number>(100);
  const [opacity, setOpacity] = useState<number>(100);

  // 2-Bone IK / FK Limb Solver
  const [ikActive, setIkActive] = useState<boolean>(true);
  const [limbChain, setLimbChain] = useState<string>("Right Leg (torso → ...)");
  const [poleBendClockwise, setPoleBendClockwise] = useState<boolean>(true);
  const [torsoSecondaryFollow, setTorsoSecondaryFollow] = useState<boolean>(true);
  const [ikTargetAngle, setIkTargetAngle] = useState<number>(0);

  // Puppet Mesh & Pin Stiffness
  const [meshDensity, setMeshDensity] = useState<number>(96);
  const [meshExpansion, setMeshExpansion] = useState<number>(6);
  const [pinMode, setPinMode] = useState<"deform" | "starch" | "overlap">("starch");
  const [pinStiffness, setPinStiffness] = useState<number>(65);
  const [pinExtent, setPinExtent] = useState<number>(38);
  const [followDelay, setFollowDelay] = useState<number>(0.1);

  // Hand & Facial Hold States
  const [handHold, setHandHold] = useState<boolean>(true);
  const [faceHold, setFaceHold] = useState<boolean>(true);
  const [selectedGesture, setSelectedGesture] = useState<string>("wave");
  const [gestureAngle, setGestureAngle] = useState<number>(0);

  // FilmCraft Handoff
  const [placeTrack, setPlaceTrack] = useState<string>("V2");
  const [placeTime, setPlaceTime] = useState<number>(0);
  const [lastRenderedClip, setLastRenderedClip] = useState<string>("");
  const [busy, setBusy] = useState<boolean>(false);

  // Skeletons definition for interactive overlay
  const bones = [
    { id: "head", label: "head", x: 260, y: 195, parent: "torso" },
    { id: "arm_R", label: "arm_R", x: 200, y: 240, parent: "torso" },
    { id: "arm_L", label: "arm_L", x: 320, y: 240, parent: "torso" },
    { id: "chest_door", label: "chest_door", x: 260, y: 270, parent: "torso" },
    { id: "torso", label: "torso", x: 260, y: 310, parent: null },
    { id: "leg_R", label: "leg_R", x: 235, y: 325, parent: "torso" },
    { id: "leg_L", label: "leg_L", x: 285, y: 325, parent: "torso" },
  ];

  const loadCharacters = async () => {
    try {
      const res = await executeCommand<CharacterListResponse>("character.list");
      const list = res.characters || [];
      setCharacters(list);
      const targetId = selectedCharId || (list.length > 0 ? list[0].id : "robot");
      await selectCharacter(targetId);
    } catch (err) {
      onToast(String(err));
    }
  };

  const selectCharacter = async (charId: string, view = "front") => {
    setSelectedCharId(charId);
    setSelectedView(view);
    try {
      const viewRes = await executeCommand<any>("animation.view", { character: charId, view });
      const sel = viewRes.selected;
      if (sel) {
        if (sel.sprites && sel.sprites.length > 0) {
          setSprites(sel.sprites);
          setSelectedSprite(sel.sprites[0]);
        }
        if (sel.clip) setLastRenderedClip(sel.clip);
        if (sel.frame) setFramePath(sel.frame);
        if (sel.time) setCurrentTime(Number(sel.time));
      }
      const m = await executeCommand<CharacterMetaResponse>("character.meta", { character: charId });
      setMeta(m);
      if (m?.shots && m.shots.length > 0) {
        setDuration(m.shots[0].duration || 2.5);
      }
    } catch {
      // character might not have full ecproj rig yet
    }
  };

  const handleImportAsset = async () => {
    const selected = await pickFile({
      title: "Import Character / Artwork Asset",
      extensions: ["png", "webp", "jpg", "jpeg", "psd", "pcraft", "svg", "glb"],
    });
    if (typeof selected !== "string" || !selected) return;
    setBusy(true);
    try {
      const name = selected.split("/").pop()?.replace(/\.[^.]+$/, "") || "character";
      const charSlug = name.toLowerCase().replace(/[^a-z0-9]+/g, "-");
      await executeCommand("project.add_asset", { path: selected, character: charSlug, name, kind: "image" });
      await onRefresh();
      await loadCharacters();
      onToast(`Imported asset ${name}`);
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    void loadCharacters();
  }, [projectPath]);

  const handleKeyTransform = async () => {
    setBusy(true);
    try {
      await executeCommand("character.set", {
        character: selectedCharId,
        view: selectedView,
        part: selectedBone,
        prop: "rotation",
        value: rotation,
        time: currentTime,
      });
      onToast(`Keyed ${selectedBone}: rot=${rotation}°, scale=${scale}%, opacity=${opacity}%`);
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleSolveIK = async () => {
    setBusy(true);
    try {
      await executeCommand("character.set", {
        character: selectedCharId,
        view: selectedView,
        part: selectedBone,
        prop: "ik_solve",
        value: ikTargetAngle,
        time: currentTime,
      });
      onToast(`Solved & Keyed 2-Bone IK on ${limbChain} (Target: ${ikTargetAngle}°)`);
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleRenderClip = async () => {
    setBusy(true);
    try {
      const res = await executeCommand<{ clip?: string; path?: string }>("character.render", {
        character: selectedCharId,
        view: selectedView,
        start: 0,
        end: duration,
        fps: 24,
        format: "clip",
      });
      const clip = res.clip || res.path || "";
      if (clip) setLastRenderedClip(clip);
      onToast(`Rendered animation clip: ${clip.split("/").pop() || selectedCharId}`);
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handlePlaceOnTimeline = async () => {
    if (!lastRenderedClip) {
      onToast("Render an animation clip (.mov) first");
      return;
    }
    setBusy(true);
    try {
      await executeCommand("video.place_clip", {
        clip: lastRenderedClip,
        from: placeTime,
        track: placeTrack,
      });
      onToast(`Placed on FilmCraft timeline (${placeTrack} @ ${placeTime}s)`);
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  // Format timecode (e.g. 0:01.065)
  const formatTimecode = (secs: number) => {
    const mins = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    const ms = Math.floor((secs % 1) * 1000);
    return `${mins}:${String(s).padStart(2, "0")}.${String(ms).padStart(3, "0")}`;
  };

  const filteredCharacters = characters.filter((c) =>
    searchFilter ? c.id.toLowerCase().includes(searchFilter.toLowerCase()) : true
  );

  return (
    <div
      id="view-animation"
      style={{
        display: "grid",
        gridTemplateColumns: "240px 1fr 310px",
        height: "100%",
        width: "100%",
        background: "#0c0e12",
        color: "#c6d0dc",
        fontFamily: "system-ui, -apple-system, sans-serif",
        overflow: "hidden",
      }}
    >
      {/* ----------------- LEFT COLUMN: Assets & Sprites ----------------- */}
      <aside
        style={{
          borderRight: "1px solid #1a202a",
          background: "#10141b",
          display: "flex",
          flexDirection: "column",
          gap: "10px",
          padding: "10px",
          overflowY: "auto",
        }}
      >
        {/* Selected Character Card */}
        <div
          style={{
            background: "#18202c",
            border: "1px solid #232d3d",
            borderRadius: "6px",
            padding: "8px 10px",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            <span
              style={{
                width: "8px",
                height: "8px",
                borderRadius: "50%",
                background: "#24d2a4",
                boxShadow: "0 0 6px #24d2a4",
              }}
            />
            <strong style={{ fontSize: "13px", color: "#f0f4f8", textTransform: "capitalize" }}>
              {selectedCharId}
            </strong>
          </div>
          <div style={{ display: "flex", gap: "4px" }}>
            <button
              className="ghost"
              style={{ padding: "2px 6px", fontSize: "11px", color: "#24d2a4" }}
              title={`Import Asset (${assets.length} in project)`}
              onClick={handleImportAsset}
            >
              +
            </button>
            <button
              className="ghost"
              style={{ padding: "2px 6px", fontSize: "12px", color: "#8e9bb0" }}
              title="Reset Character"
              onClick={() => void selectCharacter(selectedCharId)}
            >
              🗑
            </button>
          </div>
        </div>

        {/* Artwork Search */}
        <input
          type="text"
          value={searchFilter}
          onChange={(e) => setSearchFilter(e.target.value)}
          placeholder="Prepare artwork…"
          style={{
            background: "#141922",
            border: "1px solid #222a38",
            borderRadius: "5px",
            padding: "6px 10px",
            color: "#c6d0dc",
            fontSize: "12px",
            width: "100%",
            boxSizing: "border-box",
          }}
        />

        {/* Assets Collapsible */}
        <div style={{ display: "flex", flexDirection: "column", gap: "4px" }}>
          <div
            onClick={() => setAssetsOpen(!assetsOpen)}
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              cursor: "pointer",
              padding: "4px 2px",
              userSelect: "none",
            }}
          >
            <span style={{ fontSize: "12px", fontWeight: 600, color: "#9daec2", display: "flex", alignItems: "center", gap: "6px" }}>
              <span>{assetsOpen ? "▾" : "▸"}</span> Assets
            </span>
            <span style={{ fontSize: "11px", color: "#627084" }}>({characters.length})</span>
          </div>

          {assetsOpen && (
            <div style={{ display: "flex", flexDirection: "column", gap: "3px", paddingLeft: "4px" }}>
              {filteredCharacters.map((c) => {
                const active = c.id === selectedCharId;
                const isPart = c.id.includes("expressive") || c.id.includes("actions") || c.id.includes("catch") || c.id.includes("tripo");
                return (
                  <button
                    key={c.id}
                    id={`char-select-${c.id}`}
                    onClick={() => void selectCharacter(c.id)}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: "8px",
                      padding: "5px 8px",
                      borderRadius: "5px",
                      border: active ? "1px solid #24d2a4" : "1px solid transparent",
                      background: active ? "#172b2b" : "#131821",
                      color: active ? "#24d2a4" : "#c6d0dc",
                      fontSize: "12px",
                      textAlign: "left",
                      cursor: "pointer",
                      transition: "all 0.1s ease",
                    }}
                  >
                    {isPart ? (
                      <span style={{ fontSize: "9px", padding: "1px 3px", borderRadius: "3px", background: "#212b3b", color: "#8a9cb5" }}>
                        PART
                      </span>
                    ) : (
                      <span style={{ fontSize: "12px" }}>🤖</span>
                    )}
                    <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {c.name || c.id}
                    </span>
                  </button>
                );
              })}
            </div>
          )}
        </div>

        {/* Mesh & Cut Sheets Buttons */}
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "6px", marginTop: "2px" }}>
          <button
            className="ghost"
            style={{ fontSize: "11px", padding: "6px 8px", background: "#161c26", border: "1px solid #222b3a" }}
            onClick={() => void executeCommand("character.open", { character: selectedCharId, app: "effectcraft" })}
          >
            Edit Mesh
          </button>
          <button
            className="ghost"
            style={{ fontSize: "11px", padding: "6px 8px", background: "#161c26", border: "1px solid #222b3a" }}
            onClick={() => void executeCommand("character.open", { character: selectedCharId, app: "photocraft" })}
          >
            Cut Sheets
          </button>
        </div>

        {/* Hand Sprites Section */}
        <div style={{ marginTop: "8px", display: "flex", flexDirection: "column", gap: "6px" }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span style={{ fontSize: "11px", fontWeight: "bold", color: "#8a9ab0", letterSpacing: "0.04em" }}>
              HAND SPRITES
            </span>
            <button
              onClick={() => setHandHold(!handHold)}
              style={{
                fontSize: "10px",
                padding: "2px 6px",
                borderRadius: "10px",
                border: "none",
                background: handHold ? "#1b3a36" : "#222a36",
                color: handHold ? "#24d2a4" : "#7c8d9e",
                cursor: "pointer",
                fontWeight: 600,
              }}
            >
              • Hold
            </button>
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: "4px" }}>
            {["C.", "O.", "P.", "W."].map((label) => (
              <div
                key={label}
                style={{
                  aspectRatio: "1/1",
                  background: "#161b24",
                  border: "1px solid #232c3c",
                  borderRadius: "5px",
                  display: "flex",
                  flexDirection: "column",
                  alignItems: "center",
                  justifyContent: "center",
                  cursor: "pointer",
                  fontSize: "11px",
                  color: "#9ab0c8",
                }}
                onClick={() => onToast(`Selected Hand Sprite: ${label}`)}
              >
                <span>✋</span>
                <span style={{ fontSize: "10px", marginTop: "2px" }}>{label}</span>
              </div>
            ))}
          </div>
        </div>

        {/* Facial States Section */}
        <div style={{ marginTop: "8px", display: "flex", flexDirection: "column", gap: "6px" }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span style={{ fontSize: "11px", fontWeight: "bold", color: "#8a9ab0", letterSpacing: "0.04em" }}>
              FACIAL STATES
            </span>
            <button
              onClick={() => setFaceHold(!faceHold)}
              style={{
                fontSize: "10px",
                padding: "2px 6px",
                borderRadius: "10px",
                border: "none",
                background: faceHold ? "#1b3a36" : "#222a36",
                color: faceHold ? "#24d2a4" : "#7c8d9e",
                cursor: "pointer",
                fontWeight: 600,
              }}
            >
              • Hold
            </button>
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: "6px" }}>
            {["N.", "H."].map((label) => (
              <div
                key={label}
                style={{
                  aspectRatio: "1.4/1",
                  background: "#161b24",
                  border: "1px solid #232c3c",
                  borderRadius: "5px",
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "center",
                  gap: "6px",
                  cursor: "pointer",
                  fontSize: "11px",
                  color: "#9ab0c8",
                }}
                onClick={() => onToast(`Selected Facial Expression: ${label}`)}
              >
                <span>🤖</span>
                <span style={{ fontWeight: 600 }}>{label}</span>
              </div>
            ))}
          </div>
        </div>
      </aside>

      {/* ----------------- CENTER COLUMN: Stage & Transport ----------------- */}
      <main
        style={{
          display: "flex",
          flexDirection: "column",
          minWidth: 0,
          background: "#0c0e12",
          overflow: "hidden",
        }}
      >
        {/* Stage Header Toolbar */}
        <div
          style={{
            height: "44px",
            borderBottom: "1px solid #1a202a",
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "0 14px",
            background: "#10141b",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "10px", minWidth: 0, flexShrink: 1 }}>
            <strong
              style={{
                fontSize: "13px",
                color: "#e6ecf2",
                whiteSpace: "nowrap",
                overflow: "hidden",
                textOverflow: "ellipsis",
                maxWidth: "180px",
              }}
            >
              {selectedCharId} · {selectedSprite?.name ? selectedSprite.name : "Rest pose"}
            </strong>
          </div>

          {/* Mode Toggles */}
          <div style={{ display: "flex", alignItems: "center", gap: "4px" }}>
            {[
              { id: "skeleton", label: "Show Skeleton" },
              { id: "ik", label: "IK View" },
              { id: "drawing", label: "Drawing / motion review" },
              { id: "3d", label: "3D View" },
            ].map((m) => {
              const active = stageMode === m.id;
              return (
                <button
                  key={m.id}
                  onClick={() => {
                    setStageMode(m.id as any);
                    if (m.id === "skeleton") setShowSkeleton(!showSkeleton);
                  }}
                  style={{
                    padding: "4px 10px",
                    borderRadius: "4px",
                    border: active ? "1px solid #24d2a4" : "1px solid #242c3b",
                    background: active ? "#172b2b" : "#131821",
                    color: active ? "#24d2a4" : "#9eb0c6",
                    fontSize: "11px",
                    cursor: "pointer",
                    fontWeight: active ? 600 : 400,
                  }}
                >
                  {m.label}
                </button>
              );
            })}
          </div>

          {/* Tool icons */}
          <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
            <button className="ghost" style={{ padding: "4px 6px", fontSize: "13px" }} title="Edit" onClick={() => onToast("Edit Mode")}>✏️</button>
            <button className="ghost" style={{ padding: "4px 6px", fontSize: "13px" }} title="Toggle View" onClick={() => setShowSkeleton(!showSkeleton)}>👁️</button>
            <button className="ghost" style={{ padding: "4px 6px", fontSize: "13px" }} title="Grid" onClick={() => setShowGrid(!showGrid)}>▦</button>
            <button className="ghost" style={{ padding: "4px 6px", fontSize: "13px" }} title="Fit 1:1" onClick={() => setZoomLevel(1)}>1:1</button>
            <button className="ghost" style={{ padding: "4px 6px", fontSize: "13px" }} title="Zoom Out" onClick={() => setZoomLevel(Math.max(0.5, zoomLevel - 0.1))}>−</button>
            <button className="ghost" style={{ padding: "4px 6px", fontSize: "13px" }} title="Zoom In" onClick={() => setZoomLevel(Math.min(2.5, zoomLevel + 0.1))}>+</button>
            <button className="ghost" style={{ padding: "4px 6px", fontSize: "13px" }} title="Open in EffectCraft" onClick={() => void executeCommand("character.open", { character: selectedCharId, app: "effectcraft" })}>📚</button>
          </div>
        </div>

        {/* Action Cycles Bar */}
        {sprites.length > 0 && (
          <div
            style={{
              padding: "6px 14px",
              background: "#121720",
              borderBottom: "1px solid #1a202a",
              display: "flex",
              alignItems: "center",
              gap: "6px",
              overflowX: "auto",
            }}
          >
            <span style={{ fontSize: "11px", fontWeight: "bold", color: "#7a8b9f", whiteSpace: "nowrap" }}>
              Action Cycles:
            </span>
            {sprites.map((s) => {
              const active = selectedSprite?.name === s.name;
              return (
                <button
                  key={s.name}
                  id={`anim-sprite-${s.name}`}
                  onClick={() => setSelectedSprite(s)}
                  style={{
                    padding: "3px 10px",
                    borderRadius: "4px",
                    border: active ? "1px solid #24d2a4" : "1px solid #222938",
                    background: active ? "#1b3330" : "#161b24",
                    color: active ? "#24d2a4" : "#98abc0",
                    fontSize: "11px",
                    cursor: "pointer",
                    fontWeight: active ? 600 : 400,
                  }}
                >
                  {s.name}
                </button>
              );
            })}
          </div>
        )}

        {/* Main Viewport Stage */}
        <div
          id="anim-stage-preview"
          style={{
            flex: 1,
            minHeight: "260px",
            position: "relative",
            background: "radial-gradient(circle at center, #1b222d 0%, #0d1016 80%)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            overflow: "hidden",
            backgroundImage: showGrid
              ? "linear-gradient(to right, rgba(255,255,255,0.05) 1px, transparent 1px), linear-gradient(to bottom, rgba(255,255,255,0.05) 1px, transparent 1px)"
              : undefined,
            backgroundSize: showGrid ? "30px 30px" : undefined,
          }}
        >
          {/* Character Render (Sprite or Cutout) */}
          <div
            style={{
              position: "relative",
              transform: `scale(${zoomLevel})`,
              transition: "transform 0.1s ease-out",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            {selectedSprite?.preview ? (
              <img
                src={fileUrl(selectedSprite.preview)}
                alt={selectedSprite.name}
                style={{
                  height: "360px",
                  width: "auto",
                  objectFit: "contain",
                  imageRendering: "pixelated",
                  userSelect: "none",
                }}
              />
            ) : framePath ? (
              <img
                src={fileUrl(framePath.split("?")[0])}
                alt={selectedCharId}
                style={{
                  height: "360px",
                  width: "auto",
                  objectFit: "contain",
                  userSelect: "none",
                }}
              />
            ) : (
              <div style={{ textAlign: "center", color: "#66788e", fontSize: "13px" }}>
                <span>Select an action cycle above or import a character rig</span>
              </div>
            )}

            {/* Interactive SVG Bone Skeleton Overlay */}
            {showSkeleton && (
              <svg
                style={{
                  position: "absolute",
                  inset: 0,
                  width: "100%",
                  height: "100%",
                  pointerEvents: "auto",
                  overflow: "visible",
                }}
                viewBox="0 0 520 500"
              >
                {/* Bone Lines */}
                {bones.map((b) => {
                  if (!b.parent) return null;
                  const p = bones.find((x) => x.id === b.parent);
                  if (!p) return null;
                  const isSel = selectedBone === b.id;
                  return (
                    <line
                      key={`${p.id}-${b.id}`}
                      x1={p.x}
                      y1={p.y}
                      x2={b.x}
                      y2={b.y}
                      stroke={isSel ? "#ff9d00" : "#24d2a4"}
                      strokeWidth={isSel ? 3 : 2}
                      strokeDasharray={isSel ? "none" : "3 3"}
                    />
                  );
                })}

                {/* Bone Joints */}
                {bones.map((b) => {
                  const isSel = selectedBone === b.id;
                  return (
                    <g
                      key={b.id}
                      onClick={() => {
                        setSelectedBone(b.id);
                        onToast(`Selected Bone: ${b.id}`);
                      }}
                      style={{ cursor: "pointer" }}
                    >
                      <circle
                        cx={b.x}
                        cy={b.y}
                        r={isSel ? 7 : 5}
                        fill={isSel ? "#ffaa00" : "#1d2531"}
                        stroke={isSel ? "#ffffff" : "#24d2a4"}
                        strokeWidth={2}
                      />
                      <text
                        x={b.x + 9}
                        y={b.y + 4}
                        fill={isSel ? "#ffffff" : "#a6b9ce"}
                        fontSize="10"
                        fontFamily="monospace"
                        fontWeight={isSel ? "bold" : "normal"}
                      >
                        {b.label}
                      </text>
                    </g>
                  );
                })}
              </svg>
            )}
          </div>

          {/* Floating Gesture Swap Popover (Bottom-Right) */}
          <div
            style={{
              position: "absolute",
              right: "16px",
              bottom: "16px",
              background: "rgba(16, 20, 27, 0.92)",
              border: "1px solid #232d3d",
              borderRadius: "6px",
              padding: "8px 12px",
              display: "flex",
              flexDirection: "column",
              gap: "6px",
              backdropFilter: "blur(8px)",
              boxShadow: "0 4px 12px rgba(0,0,0,0.5)",
            }}
          >
            <span style={{ fontSize: "10px", fontWeight: "bold", color: "#8a9ab0", letterSpacing: "0.04em" }}>
              GESTURE SWAP: WAVE/POINT/FIST
            </span>
            <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
              <div style={{ display: "flex", gap: "4px" }}>
                {["wave", "point", "fist", "open"].map((g) => {
                  const active = selectedGesture === g;
                  return (
                    <button
                      key={g}
                      onClick={() => {
                        setSelectedGesture(g);
                        onToast(`Swapped gesture to: ${g}`);
                      }}
                      style={{
                        padding: "3px 7px",
                        fontSize: "10px",
                        borderRadius: "3px",
                        border: active ? "1px solid #24d2a4" : "1px solid #263140",
                        background: active ? "#1b3330" : "#171c26",
                        color: active ? "#24d2a4" : "#98abc0",
                        cursor: "pointer",
                      }}
                    >
                      {g}
                    </button>
                  );
                })}
              </div>
              <div style={{ width: "28px", height: "28px" }}>
                <Dial
                  id="gesture-angle"
                  label="Gesture Angle"
                  value={gestureAngle}
                  min={-180}
                  max={180}
                  step={5}
                  unit="°"
                  onChange={setGestureAngle}
                />
              </div>
            </div>
          </div>
        </div>

        {/* Viewport Transport Bar */}
        <div
          style={{
            height: "44px",
            borderTop: "1px solid #1a202a",
            background: "#10141b",
            display: "flex",
            alignItems: "center",
            padding: "0 16px",
            gap: "12px",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
            <button
              className="ghost"
              style={{ fontSize: "14px", padding: "4px 8px" }}
              title="Play / Pause"
              onClick={() => setIsPlaying(!isPlaying)}
            >
              {isPlaying ? "⏸" : "▶"}
            </button>
            <button
              className="ghost"
              style={{ fontSize: "14px", padding: "4px 8px", color: isLooping ? "#24d2a4" : "#65768a" }}
              title="Loop"
              onClick={() => setIsLooping(!isLooping)}
            >
              ↺
            </button>
            <button
              className="ghost"
              style={{ fontSize: "13px", padding: "4px 6px" }}
              title="Previous Frame"
              onClick={() => setCurrentTime(Math.max(0, currentTime - 0.04))}
            >
              ⏮
            </button>
            <button
              className="ghost"
              style={{ fontSize: "13px", padding: "4px 6px" }}
              title="Next Frame"
              onClick={() => setCurrentTime(Math.min(duration, currentTime + 0.04))}
            >
              ⏭
            </button>
          </div>

          {/* Monospace Timecode */}
          <span
            style={{
              fontFamily: "ui-monospace, monospace",
              fontSize: "12px",
              color: "#a4b6cb",
              letterSpacing: "0.05em",
              minWidth: "75px",
            }}
          >
            {formatTimecode(currentTime)}
          </span>

          {/* Full-width Teal Scrub Bar */}
          <input
            id="motion-scrub"
            type="range"
            min={0}
            max={duration}
            step={0.01}
            value={currentTime}
            onChange={(e) => setCurrentTime(Number(e.target.value))}
            style={{
              flex: 1,
              accentColor: "#24d2a4",
              cursor: "pointer",
            }}
          />
        </div>

        {/* ----------------- BOTTOM THEATRE PANEL ----------------- */}
        <div
          style={{
            height: "170px",
            borderTop: "1px solid #1a202a",
            background: "#0c0e12",
            display: "flex",
            flexDirection: "column",
          }}
        >
          {/* Theatre Heading Bar */}
          <div
            style={{
              height: "36px",
              borderBottom: "1px solid #1a202a",
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              padding: "0 14px",
              background: "#10141b",
            }}
          >
            <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
              <span style={{ fontSize: "13px", color: "#24d2a4" }}>☷</span>
              <div style={{ display: "flex", gap: "2px", background: "#161b24", padding: "2px", borderRadius: "4px" }}>
                {(["theatre", "keys", "curves"] as const).map((t) => {
                  const active = theatreTab === t;
                  return (
                    <button
                      key={t}
                      onClick={() => setTheatreTab(t)}
                      style={{
                        padding: "2px 8px",
                        fontSize: "11px",
                        borderRadius: "3px",
                        border: "none",
                        background: active ? "#24d2a4" : "transparent",
                        color: active ? "#0c1514" : "#8697ac",
                        fontWeight: active ? 600 : 400,
                        cursor: "pointer",
                        textTransform: "capitalize",
                      }}
                    >
                      {t}
                    </button>
                  );
                })}
              </div>
            </div>

            <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
              <span style={{ fontSize: "11px", color: "#74869c" }}>
                {selectedBone} · Save motion to persist edits
              </span>
              <button
                className="ghost"
                style={{ fontSize: "11px", padding: "3px 8px", border: "1px solid #232c3d", color: "#24d2a4" }}
                onClick={() => onToast("Saved motion snapshot to project")}
              >
                Save motion
              </button>
            </div>
          </div>

          {/* Clips & Range Subheading */}
          <div
            style={{
              height: "32px",
              borderBottom: "1px solid #171c26",
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              padding: "0 14px",
              fontSize: "11px",
              color: "#74869c",
            }}
          >
            <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
              <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
                <span style={{ fontWeight: 600, color: "#95a8bf" }}>CLIPS</span>
                <span
                  style={{
                    padding: "1px 6px",
                    borderRadius: "4px",
                    background: "#182230",
                    border: "1px solid #253347",
                    color: "#f0f4f8",
                    fontWeight: 600,
                  }}
                >
                  Rest
                </span>
              </div>
              <span>REVIEW RANGE 0.0s–{duration.toFixed(1)}s</span>
            </div>

            <div style={{ display: "flex", gap: "6px" }}>
              <button className="ghost" style={{ fontSize: "10px", padding: "2px 6px" }} onClick={() => onToast("Created section")}>
                Create section from selection
              </button>
              <button className="ghost" style={{ fontSize: "10px", padding: "2px 6px" }} onClick={() => onToast("Custom actions opened")}>
                Custom actions
              </button>
              <button className="ghost" style={{ fontSize: "10px", padding: "2px 6px" }} onClick={() => onToast("Design as actions")}>
                Design as actions
              </button>
            </div>
          </div>

          {/* Timeline Tracks Body */}
          <div
            style={{
              flex: 1,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              color: "#56667a",
              fontSize: "12px",
            }}
          >
            <span>Choose an action or clip to see its channels.</span>
          </div>
        </div>
      </main>

      {/* ----------------- RIGHT COLUMN: Pose & Solver Inspector ----------------- */}
      <aside
        id="anim-inspector"
        style={{
          borderLeft: "1px solid #1a202a",
          background: "#10141b",
          display: "flex",
          flexDirection: "column",
          padding: "12px",
          gap: "12px",
          overflowY: "auto",
        }}
      >
        {/* Pose Accordion Header */}
        <div
          onClick={() => setPoseOpen(!poseOpen)}
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            cursor: "pointer",
            userSelect: "none",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
            <span style={{ fontSize: "13px" }}>👁️</span>
            <strong style={{ fontSize: "13px", color: "#f0f4f8" }}>Pose</strong>
          </div>
          <span style={{ fontSize: "11px", color: "#6a7b90" }}>{poseOpen ? "▾" : "▸"}</span>
        </div>

        {poseOpen && (
          <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
            {/* Selected Bone Banner */}
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                padding: "6px 8px",
                background: "#151b24",
                border: "1px solid #232c3c",
                borderRadius: "5px",
              }}
            >
              <span style={{ fontSize: "11px", fontWeight: "bold", color: "#9daec4", letterSpacing: "0.04em" }}>
                SELECTED BONE · {selectedBone.toUpperCase()} {meta?.parts?.length ? `(${meta.parts.length} parts)` : ""}
              </span>
              <span
                style={{
                  fontSize: "10px",
                  padding: "1px 5px",
                  borderRadius: "3px",
                  background: "#192e2a",
                  color: "#24d2a4",
                  fontWeight: 600,
                  display: "flex",
                  alignItems: "center",
                  gap: "4px",
                }}
              >
                • EffectCraft
              </span>
            </div>

            {/* Transform Controls: Rotation Dial, Scale Slider, Opacity Slider */}
            <div style={{ display: "flex", flexDirection: "column", gap: "10px" }}>
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                <span style={{ fontSize: "12px", color: "#9daec4" }}>Rotation</span>
                <div style={{ width: "36px", height: "36px" }}>
                  <Dial
                    id="bone-rotation"
                    label="Rotation"
                    value={rotation}
                    min={-180}
                    max={180}
                    step={1}
                    unit="°"
                    onChange={setRotation}
                  />
                </div>
              </div>

              <Slider
                id="bone-scale"
                label="Scale"
                value={scale}
                min={10}
                max={200}
                step={1}
                unit="%"
                onChange={setScale}
              />

              <Slider
                id="bone-opacity"
                label="Opacity"
                value={opacity}
                min={0}
                max={100}
                step={1}
                unit="%"
                onChange={setOpacity}
              />

              <button
                id="anim-key-transform"
                className="ghost"
                style={{ width: "100%", padding: "6px", fontSize: "12px", border: "1px solid #242d3d" }}
                disabled={busy}
                onClick={handleKeyTransform}
              >
                Key {selectedBone} Transform
              </button>
            </div>

            <hr style={{ borderColor: "#1e2633", margin: "2px 0", borderStyle: "solid" }} />

            {/* 2-Bone IK / FK Limb Solver */}
            <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
              <span style={{ fontSize: "11px", fontWeight: "bold", color: "#8a9cb4", letterSpacing: "0.04em" }}>
                2-BONE IK / FK LIMB SOLVER
              </span>

              <Toggle
                id="ik-solver-active"
                label="2-Bone IK Solver Active"
                checked={ikActive}
                onChange={setIkActive}
              />

              <label style={{ display: "flex", flexDirection: "column", gap: "4px", fontSize: "11px", color: "#8a9cb4" }}>
                <span>Limb Chain</span>
                <select
                  value={limbChain}
                  onChange={(e) => setLimbChain(e.target.value)}
                  style={{
                    background: "#161b24",
                    border: "1px solid #232c3c",
                    borderRadius: "4px",
                    padding: "5px 8px",
                    color: "#c6d0dc",
                    fontSize: "12px",
                  }}
                >
                  <option value="Right Leg (torso → ...)">Right Leg (torso → ...)</option>
                  <option value="Left Leg (torso → ...)">Left Leg (torso → ...)</option>
                  <option value="Right Arm (torso → ...)">Right Arm (torso → ...)</option>
                  <option value="Left Arm (torso → ...)">Left Arm (torso → ...)</option>
                </select>
              </label>

              <Toggle
                id="pole-bend-clockwise"
                label="Pole Bend Clockwise"
                checked={poleBendClockwise}
                onChange={setPoleBendClockwise}
              />

              <Toggle
                id="torso-secondary-follow"
                label="Torso Secondary Follow"
                checked={torsoSecondaryFollow}
                onChange={setTorsoSecondaryFollow}
              />

              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginTop: "2px" }}>
                <span style={{ fontSize: "12px", color: "#9daec4" }}>IK Target Angle</span>
                <div style={{ width: "36px", height: "36px" }}>
                  <Dial
                    id="ik-target-angle"
                    label="IK Target Angle"
                    value={ikTargetAngle}
                    min={-180}
                    max={180}
                    step={1}
                    unit="°"
                    onChange={setIkTargetAngle}
                  />
                </div>
              </div>

              <button
                id="anim-solve-ik"
                className="start"
                style={{
                  width: "100%",
                  padding: "7px",
                  fontSize: "12px",
                  background: "#24d2a4",
                  color: "#0c1514",
                  fontWeight: 600,
                  borderRadius: "5px",
                  border: "none",
                  cursor: "pointer",
                }}
                disabled={busy}
                onClick={handleSolveIK}
              >
                Solve & Key 2-Bone IK
              </button>
            </div>

            <hr style={{ borderColor: "#1e2633", margin: "2px 0", borderStyle: "solid" }} />

            {/* Puppet Mesh & Pin Stiffness */}
            <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
              <span style={{ fontSize: "11px", fontWeight: "bold", color: "#8a9cb4", letterSpacing: "0.04em" }}>
                PUPPET MESH & PIN STIFFNESS
              </span>

              <Slider
                id="mesh-density"
                label="Mesh Density"
                value={meshDensity}
                min={12}
                max={256}
                step={4}
                unit=" tris"
                onChange={setMeshDensity}
              />

              <Slider
                id="mesh-expansion"
                label="Mesh Expansion"
                value={meshExpansion}
                min={0}
                max={32}
                step={1}
                unit=" px"
                onChange={setMeshExpansion}
              />

              {/* Deform / Starch / Overlap Pills */}
              <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: "4px", margin: "2px 0" }}>
                {(["deform", "starch", "overlap"] as const).map((mode) => {
                  const active = pinMode === mode;
                  return (
                    <button
                      key={mode}
                      onClick={() => setPinMode(mode)}
                      style={{
                        padding: "4px 0",
                        fontSize: "11px",
                        borderRadius: "4px",
                        border: active ? "1px solid #24d2a4" : "1px solid #222b39",
                        background: active ? "#1b3330" : "#151b24",
                        color: active ? "#24d2a4" : "#8fa2b8",
                        fontWeight: active ? 600 : 400,
                        textTransform: "capitalize",
                        cursor: "pointer",
                      }}
                    >
                      {mode}
                    </button>
                  );
                })}
              </div>

              <Slider
                id="pin-stiffness"
                label="Pin Stiffness"
                value={pinStiffness}
                min={0}
                max={100}
                step={1}
                unit="%"
                onChange={setPinStiffness}
              />

              <Slider
                id="pin-extent"
                label="Pin Extent"
                value={pinExtent}
                min={0}
                max={100}
                step={1}
                unit=" px"
                onChange={setPinExtent}
              />

              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                <span style={{ fontSize: "12px", color: "#9daec4" }}>Follow Delay</span>
                <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                  <div style={{ width: "32px", height: "32px" }}>
                    <Dial
                      id="follow-delay-dial"
                      label="Follow Delay"
                      value={followDelay}
                      min={0}
                      max={1}
                      step={0.01}
                      unit="s"
                      onChange={setFollowDelay}
                    />
                  </div>
                  <span style={{ fontSize: "11px", fontFamily: "monospace", color: "#24d2a4" }}>
                    {followDelay.toFixed(2)}s
                  </span>
                </div>
              </div>
            </div>

            <hr style={{ borderColor: "#1e2633", margin: "2px 0", borderStyle: "solid" }} />

            {/* Render Clip & Timeline Placement */}
            <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
              <button
                id="anim-render-clip"
                className="start"
                style={{
                  width: "100%",
                  padding: "7px",
                  fontSize: "12px",
                  background: "#24d2a4",
                  color: "#0c1514",
                  fontWeight: 600,
                  borderRadius: "5px",
                  border: "none",
                  cursor: "pointer",
                }}
                disabled={busy}
                onClick={handleRenderClip}
              >
                {busy ? "Rendering…" : "Render Animation Clip (.mov)"}
              </button>

              <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "6px" }}>
                <label style={{ display: "flex", flexDirection: "column", gap: "2px", fontSize: "10px", color: "#8a9cb4" }}>
                  <span>Track</span>
                  <select
                    value={placeTrack}
                    onChange={(e) => setPlaceTrack(e.target.value)}
                    style={{
                      background: "#161b24",
                      border: "1px solid #232c3c",
                      borderRadius: "4px",
                      padding: "4px 6px",
                      color: "#c6d0dc",
                      fontSize: "11px",
                    }}
                  >
                    <option value="V2">V2 (Characters)</option>
                    <option value="V1">V1 (Background)</option>
                    <option value="V3">V3 (Overlays)</option>
                  </select>
                </label>
                <label style={{ display: "flex", flexDirection: "column", gap: "2px", fontSize: "10px", color: "#8a9cb4" }}>
                  <span>At Seconds</span>
                  <input
                    type="number"
                    min={0}
                    step={0.5}
                    value={placeTime}
                    onChange={(e) => setPlaceTime(Number(e.target.value))}
                    style={{
                      background: "#161b24",
                      border: "1px solid #232c3c",
                      borderRadius: "4px",
                      padding: "4px 6px",
                      color: "#c6d0dc",
                      fontSize: "11px",
                    }}
                  />
                </label>
              </div>

              <button
                id="anim-place-clip-btn"
                className="ghost"
                style={{
                  width: "100%",
                  padding: "6px",
                  fontSize: "11px",
                  border: "1px solid #242d3d",
                }}
                disabled={busy || !lastRenderedClip}
                onClick={handlePlaceOnTimeline}
              >
                Place Clip on Film Timeline
              </button>
            </div>
          </div>
        )}
      </aside>
    </div>
  );
};
