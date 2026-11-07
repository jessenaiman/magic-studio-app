import { useEffect, useRef, useState } from "react";
import "../gallery.css";

type Example = {
  id: string; name: string; category: string; description: string;
  poster: string; video: string; recipe: string; nativeProject: string;
  engine: string; duration: number; commands: Array<{ id: string; params: Record<string, unknown> }>;
};

// This catalogue has no project/session inputs and never executes an authoring command.
export function GalleryView() {
  const [examples, setExamples] = useState<Example[]>([]);
  const [category, setCategory] = useState("All examples");
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [media, setMedia] = useState("");
  const video = useRef<HTMLVideoElement>(null);
  useEffect(() => {
    const controller = new AbortController();
    void fetch("/effect-gallery/catalogue.json", { signal: controller.signal })
      .then(response => { if (!response.ok) throw new Error("The effect examples could not be loaded"); return response.json() as Promise<Example[]>; })
      .then(setExamples).catch(error => { if (!controller.signal.aborted) setError(String(error)); });
    return () => controller.abort();
  }, []);
  const categories = ["All examples", ...new Set(examples.map(example => example.category))];
  const search = query.trim().toLowerCase();
  const shown = examples.filter(example => (category === "All examples" || example.category === category) &&
    (!search || `${example.name} ${example.description}`.toLowerCase().includes(search)));
  const selected = shown.find(example => example.id === selectedId) || shown[0];
  useEffect(() => {
    const controller = new AbortController(); let url = "";
    setMedia("");
    if (selected) void fetch(selected.video, { signal: controller.signal })
      .then(response => { if (!response.ok) throw new Error("The native preview is unavailable"); return response.blob(); })
      .then(blob => { if (!controller.signal.aborted) { url = URL.createObjectURL(blob); setMedia(url); } })
      .catch(error => { if (!controller.signal.aborted) setError(String(error)); });
    return () => { controller.abort(); if (url) URL.revokeObjectURL(url); };
  }, [selected?.video]);
  const play = async () => {
    if (!video.current || !media) return;
    try { video.current.currentTime = 0; await video.current.play(); } catch (error) { setError(String(error)); }
  };
  return <section id="view-gallery" className="effect-gallery" aria-label="Effect example gallery">
    <aside className="panel effect-gallery-rail" aria-label="Example categories">
      <header className="panel-head" data-look="heading"><strong>Effect gallery</strong></header>
      <div className="panel-body">
        <label htmlFor="gallery-search">Find an example</label>
        <input id="gallery-search" className="field" type="search" value={query} onChange={event => setQuery(event.target.value)} />
        <nav aria-label="Effect categories">{categories.map((name, index) => <button id={`gallery-category-${index}`} key={name} className={category === name ? "start" : "ghost"} aria-pressed={category === name} onClick={() => setCategory(name)}>{name}</button>)}</nav>
        <p className="muted">Independent examples to explore. Browsing does not use or change a song project.</p>
      </div>
    </aside>
    <main className="effect-gallery-main">
      <header><h2>Explore effects</h2><span className="muted">Native EffectCraft examples</span></header>
      {error && <p role="alert">{error}</p>}
      <div id="gallery-grid" className="effect-example-grid">{shown.map(example => <button id={`gallery-example-${example.id}`} key={example.id} className={`effect-example${selected?.id === example.id ? " selected" : ""}`} aria-pressed={selected?.id === example.id} onClick={() => { setSelectedId(example.id); setError(""); }}>
        <img src={example.poster} alt={`${example.name} native rendered example`} loading="lazy" />
        <strong>{example.name}</strong><small>{example.category}</small>
      </button>)}</div>
      {!shown.length && <p role="status">{examples.length ? "No examples match this filter." : error ? "Examples unavailable." : "Loading examples…"}</p>}
    </main>
    <aside className="panel effect-gallery-detail" aria-label="Example preview and implementation">
      <header className="panel-head" data-look="heading"><strong>{selected?.name || "Example details"}</strong></header>
      {selected && <div className="panel-body">
        <video ref={video} id="gallery-example-player" src={media || undefined} poster={selected.poster} muted playsInline controls preload="metadata" />
        <button id="gallery-replay-example" className="start" disabled={!media} onClick={() => void play()}>Replay example</button>
        <p>{selected.description}</p>
        <p className="muted">{selected.duration} seconds · rendered by {selected.engine}</p>
        <details id="gallery-native-recipe"><summary>How it is made</summary>
          <p className="muted">These native commands created the example composition. No command runs against your project from this gallery.</p>
          <pre><code>{JSON.stringify(selected.commands, null, 2)}</code></pre>
        </details>
      </div>}
    </aside>
  </section>;
}
