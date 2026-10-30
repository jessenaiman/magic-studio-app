// Navigation preserves this intentional single PTY session; it never starts one.
const backstageInvoke = window.__TAURI__.core.invoke;

class BackstageTerminalController {
  busy = false;
  closing = false;
  fit: any = null;
  generation = 0;
  input: any = null;
  resize: any = null;
  running = false;
  session: any = null;
  stopping = false;
  terminal: any = null;

  constructor() {
    window.addEventListener('backstage-visible', () => requestAnimationFrame(() => this.fitTerminal()));
    window.addEventListener('pagehide', () => this.onPageHide());
    window.addEventListener('pageshow', () => this.onPageShow());
  }

  status(message: string) {
    document.getElementById('backstage-status').textContent = message;
  }

  controls() {
    (document.getElementById('backstage-start') as HTMLButtonElement).disabled = this.busy || this.running;
    (document.getElementById('backstage-stop') as HTMLButtonElement).disabled = this.busy || !this.running;
  }

  fitTerminal() {
    const host = document.getElementById('backstage-terminal');
    if (!this.fit || !host?.getBoundingClientRect().width || !host.getBoundingClientRect().height) return;
    const dimensions = this.fit.proposeDimensions();
    if (dimensions) this.terminal.resize(Math.max(2, Math.min(500, dimensions.cols)), Math.max(2, Math.min(200, dimensions.rows)));
  }

  resizeTerminal(cols: number, rows: number) {
    const session = this.session;
    if (!this.running || session === null) return;
    let request = this.resize;
    if (request?.session === session) { request.latest = {cols, rows}; return; }
    request = {session, latest: {cols, rows}};
    this.resize = request;
    (async () => {
      while (request.latest && this.running && this.session === session) {
        const dimensions = request.latest;
        request.latest = null;
        await backstageInvoke('backstage_resize', {session, ...dimensions});
      }
    })().catch(error => {
      if (this.running && this.session === session) this.status(`Terminal resize failed: ${error}`);
    }).finally(() => { if (this.resize === request) this.resize = null; });
  }

  send(data: string, binary = false) {
    const queue = this.input, session = this.session;
    if (!this.running || session === null || !queue || queue.cancelled) return;
    if (data.length > 65536) { this.status('Terminal input is too large; send at most 64 KiB at a time.'); return; }
    const bytes = binary ? Uint8Array.from(data, char => char.charCodeAt(0)) : new TextEncoder().encode(data);
    if (queue.pending + bytes.length > 65536) { this.status('Terminal input is busy; this input was not queued.'); return; }
    queue.pending += bytes.length;
    queue.promise = queue.promise.then(async () => {
      for (let offset = 0; offset < bytes.length; offset += 8192) {
        if (queue.cancelled || !this.running || this.session !== session) return;
        await backstageInvoke('backstage_write', {session, bytes: Array.from(bytes.slice(offset, offset + 8192))});
      }
    }).catch(error => {
      if (!queue.cancelled && this.session === session) this.status(`Terminal input failed: ${error}`);
    }).finally(() => { queue.pending -= bytes.length; });
  }

  async recover() {
    ++this.generation;
    if (this.input) this.input.cancelled = true;
    this.busy = true;
    this.controls();
    try {
      const session = await backstageInvoke('backstage_current');
      this.session = session;
      this.running = session !== null;
      if (session !== null) {
        this.status('A previous Backstage session is closing or still running. Stop it before launching a new terminal.');
      }
    } catch (error) {
      this.status(`Could not inspect Backstage session: ${error}`);
    } finally {
      this.busy = false;
      this.controls();
    }
  }

  ensureTerminalInstance() {
    if (!window.MagicBackstage) throw new Error('Terminal assets unavailable. Rebuild the desktop frontend.');
    if (this.terminal) return this.terminal;
    const {Terminal, FitAddon} = window.MagicBackstage;
    const theme = getComputedStyle(document.documentElement);
    const terminal = new Terminal({
      fontSize: 13,
      scrollback: 1000,
      screenReaderMode: true,
      cursorBlink: false,
      theme: {
        background: theme.getPropertyValue('--bg').trim(),
        foreground: theme.getPropertyValue('--text').trim(),
        cursor: theme.getPropertyValue('--accent').trim(),
      },
    });
    const fit = new FitAddon();
    terminal.loadAddon(fit);
    terminal.open(document.getElementById('backstage-terminal'));
    this.terminal = terminal;
    this.fit = fit;
    terminal.onData(data => this.send(data));
    terminal.onBinary(data => this.send(data, true));
    terminal.onResize(({cols, rows}) => this.resizeTerminal(cols, rows));
    terminal.attachCustomKeyEventHandler(event => !(event.key === 'Tab' && event.shiftKey));
    new ResizeObserver(() => this.fitTerminal()).observe(document.getElementById('backstage-terminal'));
    return terminal;
  }

  createOutputChannel(generation: number, terminal: any, state: {ended: boolean, terminalError: string | null}) {
    const output = new window.__TAURI__.core.Channel();
    output.onmessage = message => {
      if (generation !== this.generation) return;
      if (message.kind === 'data') {
        if (!state.ended && !this.closing && !this.stopping) {
          this.session = message.session;
          this.running = true;
        }
        terminal.write(new Uint8Array(message.bytes), () => {
          backstageInvoke('backstage_ack', {session: message.session, sequence: message.sequence}).catch(error => {
            if (generation === this.generation && this.running) this.status(`Terminal output acknowledgement failed: ${error}`);
          });
        });
      } else if (message.kind === 'exit') {
        state.ended = true;
        this.running = false;
        this.session = null;
        this.input.cancelled = true;
        this.status(state.terminalError || message.error || `Backstage exited · ${message.success ? 'normal exit' : `status ${message.code}`}. Launch again when needed.`);
        this.controls();
      } else if (message.kind === 'error') {
        state.terminalError = message.message;
        this.status(message.message);
      }
    };
    return output;
  }

  async start() {
    if (this.busy || this.running) return;
    this.busy = true;
    this.controls();
    this.status('Launching Backstage…');
    const generation = ++this.generation;
    this.session = null;
    this.input = {promise: Promise.resolve(), pending: 0, cancelled: false};
    const channelState = {ended: false, terminalError: null};
    try {
      const terminal = this.ensureTerminalInstance();
      terminal.reset();
      this.fitTerminal();
      const output = this.createOutputChannel(generation, terminal, channelState);
      const session = await backstageInvoke('backstage_start', {cols: terminal.cols, rows: terminal.rows, output});
      if (this.closing) { await backstageInvoke('backstage_stop', {session}); return; }
      if (!channelState.ended) {
        this.session = session;
        this.running = true;
        this.status('Backstage running · q quits · Shift+Tab returns to desktop controls. Navigation preserves this session.');
        this.fitTerminal();
        this.resizeTerminal(terminal.cols, terminal.rows);
        terminal.focus();
      }
    } catch (error) {
      this.running = false;
      this.input.cancelled = true;
      this.status(`Could not launch Backstage: ${error}`);
      await this.recover();
    } finally {
      this.busy = false;
      this.controls();
    }
  }

  async stop() {
    if (this.busy || !this.running || this.session === null) return;
    const session = this.session;
    this.busy = true;
    this.running = false;
    this.stopping = true;
    if (this.input) this.input.cancelled = true;
    this.controls();
    this.status('Stopping Backstage…');
    try {
      await backstageInvoke('backstage_stop', {session});
      ++this.generation;
      this.session = null;
      this.running = false;
      this.status('Backstage stopped. Launch again when needed.');
    } catch (error) {
      this.status(`Could not stop Backstage: ${error}`);
      await this.recover();
    } finally {
      this.stopping = false;
      this.busy = false;
      this.controls();
    }
  }

  mount() {
    queueMicrotask(() => this.recover());
    const controls = window.el('div', {class: 'backstage-controls'},
      window.el('button', {id: 'backstage-start', class: 'start', disabled: true, onclick: () => this.start()}, 'Launch Backstage'),
      window.el('button', {id: 'backstage-stop', class: 'ghost', disabled: true, onclick: () => this.stop()}, 'Stop'));
    const header = new PanelHeader({id: null, title: 'Backstage · live terminal', actions: [controls]}).element;
    header.classList.add('panel-head');
    header.dataset.look = 'heading';
    return window.el('section', {id: 'backstage-panel', class: 'panel', 'aria-label': 'Interactive Backstage terminal'}, header,
      window.el('p', {id: 'backstage-status', class: 'backstage-status', role: 'status'}, 'Ready to launch. Backstage observes local production data; it never starts agents or models.'),
      window.el('div', {id: 'backstage-terminal', class: 'backstage-terminal', 'aria-label': 'Backstage terminal'}));
  }

  onPageHide() {
    this.closing = true;
    if (this.input) this.input.cancelled = true;
    const session = this.session;
    if (session !== null) backstageInvoke('backstage_stop', {session}).catch(() => {});
  }

  onPageShow() {
    if (this.closing) {
      this.closing = false;
      if (document.getElementById('backstage-panel')) this.recover();
    }
  }
}

const backstageTerminal = new BackstageTerminalController();
function backstageStatus(message: string) { backstageTerminal.status(message); }
function backstageControls() { backstageTerminal.controls(); }
function backstageFit() { backstageTerminal.fitTerminal(); }
function backstageResize(cols: number, rows: number) { backstageTerminal.resizeTerminal(cols, rows); }
function backstageSend(data: string, binary = false) { backstageTerminal.send(data, binary); }
async function backstageRecover() { return backstageTerminal.recover(); }
async function backstageStart() { return backstageTerminal.start(); }
async function backstageStop() { return backstageTerminal.stop(); }
function backstageTerminalMount() { return backstageTerminal.mount(); }
window.magicBackstageTerminal = {mount: backstageTerminalMount};
