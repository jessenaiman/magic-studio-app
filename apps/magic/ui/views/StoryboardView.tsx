import React, { useCallback, useEffect, useState } from "react";
import { executeCommand, fileUrl, pickFile } from "../api";

interface SceneInfo {
  scene: number;
  spread: number;
  fields: Record<string, string>;
}

interface ScenesResponse {
  path: string;
  scenes: SceneInfo[];
}

interface StoryboardViewProps {
  projectPath: string;
  onToast: (msg: string) => void;
}

const TEXT_GROUPS: Array<{ title: string; fields: Array<[string, string, boolean?]> }> = [
  {
    title: "Scene",
    fields: [
      ["scene_id", "Scene ID"],
      ["title", "Title"],
      ["section", "Section"],
      ["cue", "Audio / lyric cue"],
      ["time", "Time"],
      ["purpose", "Purpose / lesson", true],
      ["staging_notes", "Staging notes (one per line)", true],
      ["hand_note", "Handwritten note"],
    ],
  },
  {
    title: "Reference list",
    fields: [
      ["revision", "Revision"],
      ["overall_design", "Overall design", true],
      ["location_ref", "Location / original"],
      ["character_shared", "Character / shared"],
      ["character_lead", "Character / individual"],
      ["asset_geometry", "Asset geometry / props"],
      ["match_note", "Match note"],
      ["exact_prompt", "Exact prompt", true],
    ],
  },
];

const IMAGE_SLOTS: Array<[string, string]> = [
  ["hero_image", "Hero scene image (16:9)"],
  ["source_image", "Source"],
  ["result_image", "Result"],
  ["sketch_left", "Sketch left"],
  ["sketch_right", "Sketch right"],
];

const dirOf = (p: string) => p.slice(0, p.lastIndexOf("/"));

export const StoryboardView: React.FC<StoryboardViewProps> = ({ projectPath, onToast }) => {
  const [data, setData] = useState<ScenesResponse | null>(null);
  const [missing, setMissing] = useState(false);
  const [selected, setSelected] = useState(1);
  const [draft, setDraft] = useState<Record<string, string>>({});
  const [stamp, setStamp] = useState(0);
  const [busy, setBusy] = useState(false);

  const cache = `${dirOf(projectPath)}/.storyboard-cache`;

  const thumbs = useCallback(
    async (count: number) => {
      for (let n = 1; n <= count; n++) {
        await executeCommand("storyboard.thumbnail", { scene: n, out: `${cache}/thumb-${n}.png`, scale: 0.22 });
      }
      setStamp(Date.now());
    },
    [cache],
  );

  const load = useCallback(async () => {
    try {
      const res = await executeCommand<ScenesResponse>("storyboard.scenes");
      setData(res);
      setMissing(false);
      await thumbs(res.scenes.length);
    } catch (err) {
      const msg = String(err);
      if (/No such file|not found|os error 2/i.test(msg)) setMissing(true);
      else onToast(msg);
      setData(null);
    }
  }, [thumbs, onToast]);

  useEffect(() => {
    void load();
  }, [load]);

  const scene = data?.scenes.find((s) => s.scene === selected);

  useEffect(() => {
    setDraft({});
  }, [selected, data]);

  useEffect(() => {
    if (!scene) return;
    void executeCommand("storyboard.thumbnail", { scene: selected, out: `${cache}/page.png`, scale: 0.8 })
      .then(() => setStamp(Date.now()))
      .catch((e) => onToast(String(e)));
  }, [selected, scene === undefined, data]); // eslint-disable-line react-hooks/exhaustive-deps

  const run = async (label: string, fn: () => Promise<void>) => {
    setBusy(true);
    try {
      await fn();
      onToast(label);
    } catch (err) {
      onToast(String(err));
    } finally {
      setBusy(false);
    }
  };

  const create = () =>
    run("Created storyboard from the neutral template", async () => {
      await executeCommand("storyboard.new");
      await load();
    });

  const save = () =>
    run(`Saved scene ${selected}`, async () => {
      await executeCommand("storyboard.set", { scene: selected, fields: draft });
      await load();
    });

  const addScene = () =>
    run("Added a scene", async () => {
      const r = await executeCommand<{ scene: number }>("storyboard.add", { from: selected });
      await load();
      setSelected(r.scene);
    });

  const pickImage = (slot: string) =>
    run(`Set ${slot}`, async () => {
      const p = await pickFile({ title: "Choose image", extensions: ["png", "jpg", "jpeg", "webp"] });
      if (typeof p !== "string" || !p) return;
      await executeCommand("storyboard.set", { scene: selected, images: { [slot]: p } });
      await load();
    });

  const pickRefs = () =>
    run("Updated contact strip", async () => {
      const p = await pickFile({ title: "Choose reference images (1–12)", extensions: ["png", "jpg", "jpeg", "webp"], multiple: true });
      if (!Array.isArray(p) || p.length === 0) return;
      await executeCommand("storyboard.set", { scene: selected, refs: p.slice(0, 12) });
      await load();
    });

  const exportPdf = () =>
    run("Exported PDF", async () => {
      const pdf = `${dirOf(projectPath)}/storyboard.pdf`;
      await executeCommand("storyboard.export", { pdf });
      onToast(`Exported ${pdf}`);
    });

  if (missing) {
    return (
      <div id="view-storyboard" className="panel-empty" style={{ padding: 32 }}>
        <p>No storyboard in this project yet.</p>
        <button className="start" disabled={busy} onClick={create}>
          Create storyboard
        </button>
      </div>
    );
  }

  const value = (k: string) => draft[k] ?? scene?.fields[k] ?? "";
  const dirty = Object.keys(draft).length > 0;

  return (
    <div id="view-storyboard" style={{ display: "grid", gridTemplateColumns: "190px 1fr 380px", gap: 16, height: "100%", padding: 16 }}>
      <aside id="storyboard-rail" style={{ background: "var(--surface-2, #181c22)", borderRadius: 8, padding: 10, overflowY: "auto", display: "flex", flexDirection: "column", gap: 8 }}>
        <strong>Scenes</strong>
        {(data?.scenes ?? []).map((s) => (
          <button
            key={s.scene}
            id={`sb-nav-${s.scene}`}
            className={selected === s.scene ? "start" : "ghost"}
            style={{ padding: 4, textAlign: "left", display: "flex", flexDirection: "column", gap: 4 }}
            onClick={() => setSelected(s.scene)}
          >
            <img src={`${fileUrl(`${cache}/thumb-${s.scene}.png`)}?t=${stamp}`} alt="" style={{ width: "100%", borderRadius: 3, background: "#fff" }} />
            <span style={{ fontSize: 11 }}>{s.fields.scene_id || `Scene ${s.scene}`} · {s.fields.title}</span>
          </button>
        ))}
        <button id="sb-add" className="ghost" disabled={busy} onClick={addScene}>＋ Add scene</button>
        <button id="sb-export" className="ghost" disabled={busy} onClick={exportPdf}>Export PDF</button>
      </aside>

      <main id="sb-cards" style={{ background: "var(--surface-1, #12151a)", borderRadius: 8, padding: 12, overflow: "auto", display: "flex", alignItems: "flex-start", justifyContent: "center" }}>
        {scene ? (
          <img src={`${fileUrl(`${cache}/page.png`)}?t=${stamp}`} alt={`Scene ${selected}`} style={{ maxWidth: "100%", borderRadius: 4, background: "#fff" }} />
        ) : (
          <div className="panel-empty"><p>Loading…</p></div>
        )}
      </main>

      <aside id="storyboard-editor" style={{ background: "var(--surface-2, #181c22)", borderRadius: 8, padding: 12, overflowY: "auto", display: "flex", flexDirection: "column", gap: 10 }}>
        {TEXT_GROUPS.map((g) => (
          <section key={g.title} style={{ display: "flex", flexDirection: "column", gap: 6 }}>
            <strong>{g.title}</strong>
            {g.fields.map(([key, label, multi]) => (
              <label key={key} style={{ display: "flex", flexDirection: "column", gap: 2, fontSize: 11 }}>
                <span className="muted">{label}</span>
                {multi ? (
                  <textarea id={`sb-f-${key}`} rows={3} value={value(key)} onChange={(e) => setDraft({ ...draft, [key]: e.target.value })} />
                ) : (
                  <input id={`sb-f-${key}`} value={value(key)} onChange={(e) => setDraft({ ...draft, [key]: e.target.value })} />
                )}
              </label>
            ))}
          </section>
        ))}
        <button id="sb-save" className="start" disabled={busy || !dirty} onClick={save}>Save scene</button>
        <section style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <strong>Images</strong>
          {IMAGE_SLOTS.map(([slot, label]) => (
            <button key={slot} id={`sb-img-${slot}`} className="ghost" disabled={busy} style={{ textAlign: "left" }} onClick={() => void pickImage(slot)}>
              {label}…
            </button>
          ))}
          <button id="sb-refs" className="ghost" disabled={busy} style={{ textAlign: "left" }} onClick={() => void pickRefs()}>
            Contact strip (1–12 images)…
          </button>
        </section>
      </aside>
    </div>
  );
};
