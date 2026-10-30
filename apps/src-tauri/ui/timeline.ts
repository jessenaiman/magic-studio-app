// Theatre "Curves": every animated channel of the current clip, drawn from the engine's own
// sample (per-frame layer matrices from motion.sample) and channel summary (motion.tracks).

class TimelineController {
  seriesCache = new WeakMap<any, any>();
  width = 0;
  resizeObserver = new ResizeObserver(() => {
    const box = $('theatre-curves');
    if (box?.clientWidth && box.clientWidth !== this.width) this.render();
  });

  series(sample: any, meta: any) {
    if (this.seriesCache.has(sample)) return this.seriesCache.get(sample);
    const series = meta.layers.map((layer: any, i: number) => {
      const [px, py] = layer.pivot || [0, 0];
      const out: Record<string, number[]> = { rotation: [], scale: [], x: [], y: [], opacity: [] };
      for (const frame of sample.frames) {
        const l = frame.layers[i];
        const [a, b, c, d, e, f] = l?.matrix || [1, 0, 0, 1, 0, 0];
        out.rotation.push(Math.atan2(b, a) * 180 / Math.PI);
        out.scale.push(Math.hypot(a, b));
        out.x.push(a * px + c * py + e - px);
        out.y.push(b * px + d * py + f - py);
        out.opacity.push(l?.opacity ?? 1);
      }
      return out;
    });
    this.seriesCache.set(sample, series);
    return series;
  }

  span(values: number[]) {
    return Math.max(...values) - Math.min(...values);
  }

  moving(series: any, properties: string[]) {
    const moves: Record<string, boolean> = {
      rotation: this.span(series.rotation) > 0.05,
      position: Math.max(this.span(series.x), this.span(series.y)) > 0.05,
      scale: this.span(series.scale) > 1e-3,
      opacity: this.span(series.opacity) > 1e-3,
    };
    return properties.filter((property) => moves[property] ?? true);
  }

  curve(series: any, properties: string[]) {
    for (const kind of ['rotation', 'position', 'scale', 'opacity']) {
      if (!properties.includes(kind)) continue;
      if (kind === 'position') {
        return { kind, values: this.span(series.y) >= this.span(series.x) ? series.y : series.x };
      }
      return { kind, values: series[kind] };
    }
    return { kind: properties[0] || 'rotation', values: series.rotation };
  }

  draw(canvas: any, curve: any, keys: number[], duration: number) {
    const dpr = devicePixelRatio || 1, w = canvas.clientWidth, h = canvas.clientHeight;
    if (!w || !h) return;
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
    const ctx = canvas.getContext('2d');
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const tone = getComputedStyle(document.documentElement).getPropertyValue(uiConfig.timeline_tones[curve.kind] || '--accent').trim();
    ctx.fillStyle = tone;
    ctx.globalAlpha = 0.1;
    ctx.beginPath();
    ctx.roundRect(0, 2, w, h - 4, 3);
    ctx.fill();
    ctx.globalAlpha = 1;
    const lo = Math.min(...curve.values), hi = Math.max(...curve.values), flat = hi - lo < 1e-6;
    const y = (v: number) => (flat ? h / 2 : 4 + (h - 8) * (1 - (v - lo) / (hi - lo)));
    ctx.strokeStyle = tone;
    ctx.lineWidth = 1.4;
    ctx.lineJoin = 'round';
    ctx.beginPath();
    curve.values.forEach((v: number, i: number) => {
      const x = w * i / Math.max(1, curve.values.length - 1);
      if (i) ctx.lineTo(x, y(v));
      else ctx.moveTo(x, y(v));
    });
    ctx.stroke();
    ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--depth-gold').trim();
    for (const t of keys) {
      const x = w * Math.min(1, t / duration);
      const frame = Math.round(Math.min(1, t / duration) * (curve.values.length - 1));
      ctx.save();
      ctx.translate(x, y(curve.values[frame]));
      ctx.rotate(Math.PI / 4);
      ctx.fillRect(-3, -3, 6, 6);
      ctx.restore();
    }
  }

  seekFrom(event: MouseEvent, duration: number) {
    const r = (event.currentTarget as HTMLElement).getBoundingClientRect();
    animationPause();
    motion.time = Math.max(0, Math.min(1, (event.clientX - r.left) / r.width)) * duration;
    animationDraw();
  }

  tick(at: number, text: string, end = false) {
    const node = el('span', end ? { class: 'end' } : {}, text);
    node.style.setProperty('--tick-position', `${at}%`);
    return node;
  }

  ruler(duration: number) {
    return el('div', { class: 'theatre-ruler' },
      ...Array.from({ length: Math.floor(duration) + 1 }, (_, s) => this.tick(100 * s / duration, `${s}s`)),
      duration % 1 ? this.tick(100, `${duration.toFixed(1)}s`, true) : null);
  }

  rows(meta: any, series: any[], track: any) {
    return (track?.channels || [])
      .map((channel: any) => {
        const index = meta.layers.findIndex((l: any) => l.id === channel.layer);
        return { channel, index, properties: index < 0 ? [] : this.moving(series[index], channel.properties) };
      })
      .filter((row: any) => row.properties.length);
  }

  buildRow(row: any, { meta, series, duration, drawn }: any) {
    const { channel, index, properties } = row;
    const layer = meta.layers[index], name = layer.label || layer.id, picked = motion.selected === layer.id;
    const curve = this.curve(series[index], properties);
    const canvas = el('canvas', { class: 'theatre-curve', 'aria-hidden': 'true', onclick: (event: MouseEvent) => this.seekFrom(event, duration) });
    drawn.push([canvas, curve, channel.keys?.[curve.kind] || []]);
    const node = el('div', { class: `theatre-row${picked ? ' selected' : ''}` },
      el('button', {
        class: 'theatre-channel', title: `Select ${name}`, 'aria-pressed': String(picked),
        onclick: () => { motion.selected = layer.id; animationControls(); animationDraw(); updateMagicSelection(); },
      }, `${name} · ${properties.join(' + ')}`),
      el('span', { class: 'theatre-motion' }, channel.summary),
      canvas);
    node.style.setProperty('--tone', `var(${uiConfig.timeline_tones[curve.kind] || '--accent'})`);
    return node;
  }

  note(meta: any, rows: any[]) {
    const picked = rows.find(({ index }) => meta.layers[index].id === motion.selected);
    return picked
      ? `${meta.layers[picked.index].label || picked.channel.layer} / ${picked.properties.join(' + ')} · ${picked.channel.summary}`
      : `${rows.length} animated parts · select one to follow it`;
  }

  render() {
    const box = $('theatre-curves');
    if (!box || typeof box.replaceChildren !== 'function') return;
    if (!box.dataset.observed) { box.dataset.observed = 'true'; this.resizeObserver.observe(box); }
    const meta = motion.meta, sample = motion.sample;
    const track = meta?.tracks?.find((t: any) => t.id === motion.track);
    if ($('theatre-info')) {
      $('theatre-info').textContent = sample ? `Range 0.00–${sample.duration.toFixed(2)}s · ${sample.fps} fps · ${sample.loop ? 'loop' : 'once'}` : '';
    }
    if (!meta || !sample) {
      box.replaceChildren(el('p', { class: 'panel-empty inline muted' }, 'Choose an action or clip to see its channels.'));
      return;
    }
    const series = this.series(sample, meta);
    const rows = motion.rest ? [] : this.rows(meta, series, track);
    if (!rows.length) {
      box.replaceChildren(el('p', { class: 'panel-empty inline muted' }, motion.rest ? 'Rest pose · nothing moves. Choose an action or clip.' : 'This clip animates no channels.'));
      return;
    }
    const duration = sample.duration || 1;
    const drawn: any[] = [];
    const lines = rows.map((row: any) => this.buildRow(row, { meta, series, duration, drawn }));
    box.replaceChildren(
      el('div', { class: 'theatre-grid' },
        el('div', { id: 'theatre-ruler' }, el('span', {}, 'Layer / channel'), el('span', {}, 'Motion'), this.ruler(duration)),
        ...lines,
        el('div', { class: 'theatre-playhead', id: 'theatre-playhead' })),
      el('div', { class: 'theatre-foot muted' }, el('span', {}, this.note(meta, rows)), el('span', {}, 'Curves show the clip’s source motion · Keys edits Theatre rotation')),
    );
    for (const [canvas, curve, keys] of drawn) this.draw(canvas, curve, keys, duration);
    this.width = box.clientWidth;
    this.seek(motion.time);
  }

  seek(time: number) {
    const head = $('theatre-playhead');
    if (!head || !motion.sample) return;
    head.style.setProperty('--at', String(Math.max(0, Math.min(1, time / (motion.sample.duration || 1)))));
  }
}

const timelineController = new TimelineController();
const timelineSeriesCache = timelineController.seriesCache;
const timelineResize = timelineController.resizeObserver;

function timelineSeries(sample: any, meta: any) { return timelineController.series(sample, meta); }
const timelineSpan = (values: number[]) => timelineController.span(values);
function timelineMoving(series: any, properties: string[]) { return timelineController.moving(series, properties); }
function timelineCurve(series: any, properties: string[]) { return timelineController.curve(series, properties); }
function timelineDraw(canvas: any, curve: any, keys: number[], duration: number) {
  timelineController.draw(canvas, curve, keys, duration);
}
function timelineSeekFrom(event: MouseEvent, duration: number) { timelineController.seekFrom(event, duration); }
function timelineTick(at: number, text: string, end = false) { return timelineController.tick(at, text, end); }
function timelineRuler(duration: number) { return timelineController.ruler(duration); }
function timelineRows(meta: any, series: any[], track: any) { return timelineController.rows(meta, series, track); }
function timelineRow(row: any, ctx: any) { return timelineController.buildRow(row, ctx); }
function timelineNote(meta: any, rows: any[]) { return timelineController.note(meta, rows); }
function timelineRender() { timelineController.render(); }
function timelineSeek(time: number) { timelineController.seek(time); }
