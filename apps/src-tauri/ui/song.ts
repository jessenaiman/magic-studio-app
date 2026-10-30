// Song playback, lyrics and read-along for the open .magic project.
class SongView {
  audio: HTMLAudioElement | null = null;
  path: string | null = null;
  words: any[] = [];

  section(id: string, title: string, meta: string, done: boolean, sectionState: string, body: any[]) {
    const head = window.companionBoxes.dot(done ? "ready" : "needs-attention", "", `song-${id}`);
    head.title = done ? "Ready" : "Needs work";
    return el("div", { class: "song-section", id: `song-${id}`, "data-section": id, "data-state": sectionState },
      el("div", { class: "panel-head" },
        head,
        el("span", { class: "title" }, title), el("span", { class: "meta" }, meta || "")),
      ...body);
  }

  transcriptSummary(doc: any, ok: boolean) {
    const words = doc?.words || [];
    if (!words.length) return "No timed words yet";
    const end = doc.recording_duration_sec ?? words.at(-1).end;
    return `${words.length} words · ${this.clockTime(end)} · ${ok ? "timing ok" : "timing needs review"}`;
  }

  clockTime(seconds: number) {
    const s = Math.max(0, Math.floor(seconds || 0));
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  }

  songAudio(path: string) {
    if (this.path !== path) {
      this.audio?.pause();
      this.audio = new Audio(convertFileSrc(path));
      this.path = path;
      for (const type of ["play", "pause", "ended"]) this.audio.addEventListener(type, () => this.renderPlay());
      this.audio.addEventListener("timeupdate", () => this.follow());
    }
    return this.audio!;
  }

  stop() {
    this.audio?.pause();
    this.audio = null;
    this.path = null;
  }

  playButton(path: string) {
    const button = el("button", { class: "icon song-play", id: "song-play", "data-path": path,
      onclick: (e: Event) => {
        e.stopPropagation(); // play/pause never opens or closes the Song section
        const audio = this.songAudio(path);
        if (audio.paused) audio.play().catch((error: any) => toast(`Cannot play the song: ${error.message || error}`));
        else audio.pause();
      } });
    this.renderPlay(button);
    return button;
  }

  renderPlay(button: any = $("song-play")) {
    if (!button?.dataset) return;
    const playing = this.path === button.dataset.path && this.audio && !this.audio.paused;
    const label = playing ? "Pause song" : "Play song";
    button.setAttribute("aria-label", label); button.title = label; button.setAttribute("aria-pressed", String(!!playing));
    button.innerHTML = playing ? ICONS.pause : ICONS.play;
  }

  lyricModel(lyrics: any, doc: any, asHeard = false) {
    if (!asHeard && lyrics?.source === "canonical") return { canonical: true, sections: (lyrics.sections || []).map((s: any) => ({ label: s.label,
      lines: s.lines.map((l: any) => ({ text: l.text, start: l.start ?? null, words: l.words || [] })) })) };
    const words = (doc?.words || []).filter((w: any) => Number.isFinite(w.start) && Number.isFinite(w.end));
    const lines: any[][] = [];
    let line: any[] = [];
    words.forEach((w: any, i: number) => {
      if (line.length && (w.start - words[i - 1].end >= 0.6 || line.length >= 10)) { lines.push(line); line = []; }
      line.push(w);
    });
    if (line.length) lines.push(line);
    return { canonical: false, sections: [{ label: null, lines: lines.map((l) => ({ text: l.map((w) => w.text).join(" "), start: l[0].start, words: l })) }] };
  }

  async loadLyrics() {
    const project = state.project;
    try {
      const lyrics = await invoke("engine_command", { id: "song.lyrics", params: {} });
      if (state.project === project) { state.lyrics = lyrics; renderBlocks(); }
    } catch (_error) { if (state.project === project) state.lyrics = null; }
  }

  async attachOriginal(event?: any) {
    const path = event?.currentTarget?.uiFiles?.[0] || await dialog.open({ title: "Attach the artist's original lyrics or liner notes" });
    if (!path) return;
    try {
      await designCommand("workshop.add", { kind: "liner_notes", paths: [path] });
      await this.loadLyrics();
      toast("Attached to liner notes; the original file is unchanged");
    } catch (error) { toast(String(error)); }
  }

  async importTranscript(event?: any) {
    const path = event?.currentTarget?.uiFiles?.[0] || await dialog.open({ title: "Import existing timed lyrics", filters: [{ name: "Transcript JSON", extensions: ["json"] }] });
    if (!path) return;
    try {
      await designCommand("workshop.import_transcript", { path });
      toast("Timed lyrics imported. Review the words before creating scene pages.");
    } catch (error) { toast(String(error)); }
  }

  lyricSheet(model: any) {
    return el("div", { class: "lyric-sheet" }, ...model.sections.map((section: any) => el("div", { class: "lyric-stanza" },
      section.label ? el("div", { class: "stanza-label" }, section.label) : null,
      ...section.lines.map((line: any) => el("div", { class: "lyric-line" }, el("span", { class: "lyric-text" }, line.text),
        line.start != null ? el("span", { class: "cue", title: "Sung at" }, this.clockTime(line.start)) : null)))));
  }

  lyricSourceLabel(model: any, lyrics: any) {
    if (!model.canonical) return "From the transcript, as heard and not corrected · no artist lyric sheet attached";
    const artist = lyrics.artist ? `${lyrics.artist} · ` : "";
    const timing = lyrics.timing ? " · timing from the recording" : "";
    return `${artist}the artist's lyric sheet${timing}`;
  }

  lyricOriginalActions(original: any, name: (p: string) => string) {
    if (original?.exists) {
      return el("button", { id: "song-open-original", title: original.path, onclick: () => openPath(original.path) }, `Open artist's original · ${name(original.path)}`);
    }
    return [
      el("span", { class: "not-linked" }, original ? `Original not found: ${name(original.path)}` : "Original not linked"),
      el("button", { id: "song-attach-original", class: "ghost", onclick: (e: any) => this.attachOriginal(e) }, "Attach original…"),
    ];
  }

  lyricsBody(lyrics: any, doc: any, liner: string[], songPath: string) {
    const model = this.lyricModel(lyrics, doc);
    const original = lyrics?.original;
    const name = (path: string) => String(path).split("/").pop() || "";
    const hasLines = model.sections.some((section: any) => section.lines.length);
    if (!hasLines && !original && !liner.length) {
      return [el("button", { id: "song-attach-original", class: "ghost", title: "Attach the original lyric sheet", onclick: (e: any) => this.attachOriginal(e) }, "Attach lyric sheet…")];
    }
    const note = model.canonical && lyrics.artist_note
      ? el("blockquote", { class: "artist-note" }, el("p", {}, lyrics.artist_note), el("cite", {}, lyrics.artist || "The artist"))
      : null;
    return [
      el("p", { class: "lyric-source" }, this.lyricSourceLabel(model, lyrics)),
      note,
      el("div", { class: "lyric-actions" },
        this.lyricOriginalActions(original, name),
        hasLines ? el("button", { id: "song-read-along", onclick: () => this.openReader(lyrics, doc, songPath) }, "Read along") : null),
      this.lyricSheet(model),
      el("div", { class: "group-label" }, "Liner notes"),
      liner.length ? liner.map(fileButton) : el("div", { class: "why" }, "None attached yet."),
    ];
  }

  buildReaderLine(line: any, songPath: string, seek: (t: number) => void, words: any[]) {
    const timeBtn = line.start != null
      ? el("button", { class: "reader-time", title: songPath ? "Play from here" : "", "aria-label": `Seek to ${this.clockTime(line.start)}`, onclick: () => seek(line.start) }, this.clockTime(line.start))
      : null;
    const spans = line.words.length
      ? line.words.map((w: any) => {
          words.push(w);
          return el("span", { class: "reader-word", "data-index": String(words.length - 1), title: `${(+w.start).toFixed(2)}–${(+w.end).toFixed(2)} s` }, `${w.text} `);
        })
      : [el("span", {}, line.text)];
    return el("p", { class: "reader-line" }, timeBtn, ...spans);
  }

  openReader(lyrics: any, doc: any, songPath: string, asHeard = false) {
    const model = this.lyricModel(lyrics, doc, asHeard);
    const seek = (t: number) => { if (songPath && t != null) this.songAudio(songPath).currentTime = t; };
    const words: any[] = [];
    const body = el("div", { class: "reader-body" }, ...model.sections.map((section: any) => el("div", { class: "lyric-stanza" },
      section.label ? el("div", { class: "stanza-label" }, section.label) : null,
      ...section.lines.map((line: any) => this.buildReaderLine(line, songPath, seek, words)))));
    const title = model.canonical ? "Lyrics" : "Transcript";
    const pane = el("section", { id: "magic-reader", "data-drawer": "reader", "aria-label": `${title}, read only` },
      el("div", { id: "magic-reader-head" }, el("span", {}, title),
        el("span", { class: "magic-target" }, model.canonical ? "artist's sheet · read only" : "as heard · read only"),
        el("button", { id: "magic-reader-close", "aria-label": `Close the ${title.toLowerCase()}`, onclick: () => this.closeReader() }, "Close")),
      body);
    $("magic-reader")?.remove();
    $("magic-workstation")?.append(pane);
    window.workshopDrawers?.open("reader");
    this.words = words;
    this.follow();
  }

  closeReader() {
    if (window.workshopDrawers?.active === "reader") window.workshopDrawers.close();
    $("magic-reader")?.remove();
    this.words = [];
  }

  follow() {
    const pane = $("magic-reader");
    if (!pane || !this.audio) return;
    const t = this.audio.currentTime;
    const index = this.words.findIndex((w) => t >= w.start && t < w.end);
    pane.querySelector(".reader-word.now")?.classList.remove("now");
    if (index >= 0) pane.querySelector(`.reader-word[data-index="${index}"]`)?.classList.add("now");
  }
}

const songView = new SongView();
const songPlayer = songView;
const lyricReader = songView;
window.songView = songView;

function songSection(id: string, title: string, meta: string, done: boolean, sectionState: string, body: any[]) {
  return songView.section(id, title, meta, done, sectionState, body);
}
function transcriptSummary(doc: any, ok: boolean) { return songView.transcriptSummary(doc, ok); }
function clockTime(seconds: number) { return songView.clockTime(seconds); }
function songAudio(path: string) { return songView.songAudio(path); }
function songStop() { songView.stop(); }
function songPlayButton(path: string) { return songView.playButton(path); }
function renderSongPlay(button: any = $("song-play")) { songView.renderPlay(button); }
function lyricModel(lyrics: any, doc: any, asHeard = false) { return songView.lyricModel(lyrics, doc, asHeard); }
async function loadSongLyrics() { return songView.loadLyrics(); }
async function attachOriginal(event?: any) { return songView.attachOriginal(event); }
async function importProjectTranscript(event?: any) { return songView.importTranscript(event); }
function lyricSheet(model: any) { return songView.lyricSheet(model); }
function lyricsBody(lyrics: any, doc: any, liner: string[], songPath: string) {
  return songView.lyricsBody(lyrics, doc, liner, songPath);
}
function lyricReaderOpen(lyrics: any, doc: any, songPath: string, asHeard = false) {
  songView.openReader(lyrics, doc, songPath, asHeard);
}
function transcriptReaderClose() { songView.closeReader(); }
function readerFollow() { songView.follow(); }
