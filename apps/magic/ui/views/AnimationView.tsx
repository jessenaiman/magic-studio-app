import React, { useEffect, useState } from "react";
import { executeCommand, fileUrl, pickFile, ProjectAsset } from "../api";

interface AnimationViewProps {
  projectPath: string;
  assets: ProjectAsset[];
  onRefresh: () => Promise<void>;
  onToast: (msg: string) => void;
}

interface AnimationState {
  characters: Array<{ id: string; name?: string; views: string[] }>;
  selected: { character: string; view: string; time: number; frame?: string; clip?: string } | null;
}

const panel = { background: "var(--surface-2, #181c22)", borderRadius: "8px", padding: "12px", display: "flex", flexDirection: "column", gap: "10px", overflowY: "auto" } as const;
const field = { display: "grid", gap: "4px", fontSize: "12px" } as const;

export const AnimationView: React.FC<AnimationViewProps> = ({ projectPath, assets, onRefresh, onToast }) => {
  const [state, setState] = useState<AnimationState | null>(null);
  const [duration, setDuration] = useState(4);
  const [track, setTrack] = useState("V2");
  const [from, setFrom] = useState(0);
  const [busy, setBusy] = useState(false);
  const selected = state?.selected ?? null;
  const target = selected ? { character: selected.character, view: selected.view } : {};

  const show = (params: Record<string, unknown> = target) =>
    executeCommand<AnimationState>("animation.view", params).then(setState).catch((error) => onToast(String(error)));

  const run = async (command: string, params: Record<string, unknown>, done: string) => {
    setBusy(true);
    try {
      await executeCommand(command, params);
      await show();
      onToast(done);
    } catch (error) {
      onToast(String(error));
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    void show({});
  }, [projectPath]);

  const importAsset = async () => {
    const path = await pickFile({ title: "Import Character / Artwork Asset", extensions: ["png", "webp", "jpg", "jpeg", "psd", "pcraft", "svg", "glb"] });
    if (typeof path !== "string" || !path) return;
    await run("character.import", { path }, "Imported");
    await onRefresh();
  };

  return (
    <div id="view-animation" style={{ display: "grid", gridTemplateColumns: "280px 1fr 320px", gap: "12px", height: "100%", padding: "12px" }}>
      <aside style={panel}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <strong>Characters & Assets</strong>
          <button id="anim-import-asset" className="start" disabled={busy} onClick={() => void importAsset()}>+ Add Asset</button>
        </div>
        {state?.characters.map((c) => (
          <button key={c.id} id={`char-select-${c.id}`} className={c.id === selected?.character ? "start" : "ghost"} style={{ textAlign: "left", justifyContent: "space-between" }} onClick={() => void show({ character: c.id })}>
            <span>{c.name || c.id}</span>
            <span className="muted" style={{ fontSize: "11px" }}>{c.views.join(", ")}</span>
          </button>
        ))}
        {assets.map((asset) => (
          <button key={asset.id} className="ghost" style={{ textAlign: "left", fontSize: "12px" }} disabled={!asset.path} onClick={() => void executeCommand("workspace.open", { path: asset.path }).catch((error) => onToast(String(error)))}>
            {asset.name}
          </button>
        ))}
      </aside>

      <main style={{ background: "var(--surface-1, #12151a)", borderRadius: "8px", padding: "16px", display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "space-between", gap: "12px", overflow: "auto" }}>
        <div style={{ width: "100%", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <strong>Animation Stage {selected ? `· ${selected.character} (${selected.view})` : ""}</strong>
          {selected && (
            <div style={{ display: "flex", gap: "6px" }}>
              <button id="anim-open-effectcraft" className="start" onClick={() => void run("character.open", { ...target, app: "effectcraft" }, "Opened in EffectCraft")}>Open in EffectCraft</button>
              <button id="anim-open-photocraft" className="ghost" onClick={() => void run("character.open", { ...target, app: "photocraft" }, "Opened in PhotoCraft")}>Open in PhotoCraft</button>
            </div>
          )}
        </div>
        <div id="anim-stage-preview" style={{ width: "100%", maxWidth: "760px", aspectRatio: "16 / 9", background: "#0d1016", border: "1px solid var(--border, #2a313c)", borderRadius: "8px", display: "flex", alignItems: "center", justifyContent: "center", overflow: "hidden" }}>
          {selected?.frame
            ? <img src={fileUrl(selected.frame)} alt={selected.character} style={{ maxWidth: "100%", maxHeight: "100%", objectFit: "contain" }} />
            : <span className="muted">{selected ? "Open in EffectCraft to animate" : "Select or import a character asset on the left"}</span>}
        </div>
        <div style={{ width: "100%", display: "flex", alignItems: "center", gap: "10px" }}>
          <span className="muted" style={{ fontSize: "12px" }}>Time ({(selected?.time ?? 0).toFixed(2)}s)</span>
          <input id="anim-time-scrub" type="range" min={0} max={duration} step={0.1} value={selected?.time ?? 0} onChange={(e) => void show({ ...target, time: Number(e.target.value) })} style={{ flex: 1 }} />
        </div>
      </main>

      <aside id="anim-inspector" style={panel}>
        <label style={field}>
          <span>Duration (s)</span>
          <input id="anim-shot-duration" type="number" min={1} max={60} value={duration} onChange={(e) => setDuration(Number(e.target.value))} />
        </label>
        <button id="anim-render-clip" className="start" disabled={busy || !selected} onClick={() => void run("character.render", { ...target, end: duration, format: "clip" }, "Rendered clip")}>
          {busy ? "Working…" : "Render Animation Clip (.mov)"}
        </button>
        <hr style={{ borderColor: "var(--border, #2a313c)", width: "100%" }} />
        <strong>Handoff to FilmCraft Timeline</strong>
        <span id="anim-clip-path" className="muted" style={{ fontSize: "12px", wordBreak: "break-all" }}>{selected?.clip ?? "No rendered clip yet"}</span>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "8px" }}>
          <label style={field}>
            <span>Track</span>
            <select id="anim-place-track" value={track} onChange={(e) => setTrack(e.target.value)}>
              <option value="V2">V2 (Characters)</option>
              <option value="V1">V1 (Backgrounds)</option>
              <option value="V3">V3 (Overlays)</option>
            </select>
          </label>
          <label style={field}>
            <span>At Seconds</span>
            <input id="anim-place-from" type="number" min={0} step={0.5} value={from} onChange={(e) => setFrom(Number(e.target.value))} />
          </label>
        </div>
        <button id="anim-place-clip-btn" className="start" disabled={busy || !selected?.clip} onClick={() => void run("video.place_clip", { clip: selected?.clip, from, track }, `Placed on ${track} at ${from}s`)}>
          Place Clip on Film Timeline
        </button>
      </aside>
    </div>
  );
};
