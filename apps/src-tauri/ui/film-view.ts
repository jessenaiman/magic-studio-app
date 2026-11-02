// The Studio tab: the film as FilmCraft holds it. A player for the FilmCraft preview with sound,
// and the timeline's layers (tracks and clips) from `video.timeline`. Every button is a command:
// video.assemble, video.preview, video.open (FilmCraft), video.export.

class FilmView {
  root: HTMLElement | null = null;
  timeline: any = null;
  status: any = null;
  busy = false;
  loaded = '';

  async open() {
    const view = $('view-studio');
    view.replaceChildren();
    const status = el('p', { id: 'film-status', class: 'muted', role: 'status' }, 'Loading the film…');
    const player = el('video', { id: 'film-player', class: 'film-player', controls: true, preload: 'auto' });
    const layers = el('div', { id: 'film-layers', class: 'film-layers', 'aria-label': 'Film layers' });
    const button = (id: string, label: string, title: string, run: () => Promise<any>, cls = 'ghost') =>
      el('button', { id, class: cls, title, 'data-commands': id.replace('film-', 'video.'), onclick: () => this.run(label, run) }, label);
    const head = new PanelHeader({
      id: 'film-head', title: 'Film',
      actions: [
        button('film-assemble', 'Assemble', 'Build the FilmCraft timeline from the song, scenes, characters and lyrics', () => motionCommand('video.assemble', { export: false })),
        button('film-preview', 'Preview', 'Render a half-size preview with sound to watch here', () => motionCommand('video.preview'), 'start'),
        button('film-open', 'Open in FilmCraft', 'Edit the timeline in FilmCraft', () => motionCommand('video.open')),
        button('film-export', 'Export', 'Render the final film to exports/', () => motionCommand('video.export')),
      ],
    });
    view.append(el('div', { class: 'film-view' }, head.element, player, layers, status));
    this.root = view;
    this.status = status;
    await this.refresh();
  }

  async run(label: string, run: () => Promise<any>) {
    if (this.busy) return;
    this.busy = true;
    this.status.textContent = `${label}…`;
    try { await run(); await this.refresh(); this.status.textContent = `${label} done`; }
    catch (error) { this.status.textContent = String(error); }
    finally { this.busy = false; }
  }

  /** WebKitGTK's media player cannot stream Tauri's asset:// scheme, so the preview is read
   *  into a blob URL the player can open; the previous one is released. */
  async load(player: HTMLVideoElement, url: string) {
    if (this.loaded === url) return;
    const blob = await (await fetch(url)).blob();
    if (player.src.startsWith('blob:')) URL.revokeObjectURL(player.src);
    player.src = URL.createObjectURL(blob);
    this.loaded = url;
  }

  async refresh() {
    const film = await motionCommand('video.film');
    const player = $('film-player') as HTMLVideoElement;
    if (film.preview.exists) {
      await this.load(player, `${convertFileSrc(film.preview.path)}?v=${film.preview.modified}`);
      this.status.textContent = film.preview_stale ? 'The preview is older than the timeline: render a new preview' : 'Preview ready';
    } else {
      player.removeAttribute('src');
      this.status.textContent = film.timeline.exists ? 'No preview yet: render one' : 'No film yet: Assemble builds the timeline';
    }
    if (!film.timeline.exists) { $('film-layers').replaceChildren(); return; }
    this.timeline = await motionCommand('video.timeline');
    this.drawLayers(player);
  }

  /** The tracks and clips on a Konva canvas: one row per track, clip blocks in time, a playhead. */
  drawLayers(player: HTMLVideoElement) {
    const host = $('film-layers');
    host.replaceChildren();
    const total = this.timeline.duration || 1, rowH = 28, labelW = 44;
    const width = host.clientWidth || 800, height = this.timeline.tracks.length * rowH + 4;
    const css = getComputedStyle(document.documentElement);
    const color = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback;
    const stage = new Konva.Stage({ container: host, width, height });
    const layer = new Konva.Layer();
    const x = (t: number) => labelW + (t / total) * (width - labelW);
    this.timeline.tracks.forEach((track: any, row: number) => {
      const y = row * rowH;
      layer.add(new Konva.Text({ x: 4, y: y + 8, text: track.label, fill: color('--muted', '#999'), fontSize: 12 }));
      for (const c of track.clips) {
        const block = new Konva.Group({ x: x(c.start), y: y + 3 });
        const w = Math.max(2, x(c.end) - x(c.start));
        block.add(new Konva.Rect({ width: w, height: rowH - 6, cornerRadius: 3,
          fill: track.kind === 'audio' ? color('--surface-2', '#2a3340') : color('--accent', '#3fb8a8'), opacity: c.enabled === false ? 0.3 : 0.75 }));
        block.add(new Konva.Text({ x: 4, y: 6, width: w - 8, text: c.name, fontSize: 11, fill: color('--text', '#fff'), wrap: 'none', ellipsis: true }));
        block.on('click tap', () => { player.currentTime = c.start; });
        layer.add(block);
      }
    });
    const playhead = new Konva.Line({ points: [0, 0, 0, height], stroke: color('--warn', '#ffb020'), strokeWidth: 2, listening: false });
    layer.add(playhead);
    stage.add(layer);
    const follow = () => { playhead.x(x(player.currentTime)); layer.batchDraw(); };
    player.ontimeupdate = follow;
    follow();
  }
}

const filmView = new FilmView();
