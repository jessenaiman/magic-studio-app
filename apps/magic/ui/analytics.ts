function mayhemTime(seconds: unknown) { return typeof seconds === 'number' ? new Date(seconds * 1000).toLocaleString() : 'Unknown'; }

// Display Lizard's native XML labels and values; no custom thresholds or grading.
function mayhemLizard(report: any, type: string) {
  const tool = report?.tools?.find((row: any) => row.tool === 'lizard');
  if (typeof tool?.stdout !== 'string') return [];
  const xml = new DOMParser().parseFromString(tool.stdout, 'application/xml');
  if (xml.querySelector('parsererror')) return [];
  const measure = [...xml.querySelectorAll('measure')].find(node => node.getAttribute('type') === type);
  if (!measure) return [];
  const labels = [...measure.querySelectorAll('labels > label')].map(node => node.textContent);
  return [...measure.querySelectorAll('item')].map(node => ({name: node.getAttribute('name'),
    values: Object.fromEntries([...node.querySelectorAll('value')].map((value, i) => [labels[i], value.textContent?.trim() ? Number(value.textContent) : NaN]))}));
}

const mayhemBundles = new Map<string, Promise<void>>();
function mayhemBundle(file: string): Promise<void> {
  if (!mayhemBundles.has(file)) mayhemBundles.set(file, new Promise((resolve, reject) => {
    const script = document.createElement('script');
    script.src = `vendor/${file}.bundle.js`;
    script.onload = () => resolve();
    script.onerror = () => { script.remove(); mayhemBundles.delete(file); reject(new Error(`Could not load ${file}. Rebuild frontend bundles.`)); };
    document.head.append(script);
  }));
  return mayhemBundles.get(file);
}

class AnalyticsViewController {
  data: any = null;
  busy = false;
  error = '';
  profile = 'quick';
  charts: {dispose(): void}[] = [];

  clearChart() { this.charts.forEach(chart => chart.dispose()); this.charts = []; }

  async render(scan = false) {
    if (this.busy) return;
    this.busy = true;
    this.error = '';
    this.renderSurface();
    try {
      if (scan) await motionCommand('mayhem.scan', {quick: this.profile === 'quick'});
      this.data = await motionCommand('mayhem.evidence');
    } catch (error) { this.error = String(error); }
    finally { this.busy = false; this.renderSurface(); }
  }

  async plot(node: HTMLElement, rows: any[], unit: string) {
    if (!rows.length) { node.textContent = 'No native measurements in the saved report.'; return; }
    try {
      await mayhemBundle('mayhem-charts');
      if (node.isConnected && state.view === 'analytics') this.charts.push((window as any).magicMayhemCharts.mount(
        node, rows, 'bar', getComputedStyle(node).getPropertyValue('--accent').trim(), unit));
    } catch (error) { node.textContent = String(error); }
  }

  renderSurface() {
    this.clearChart();
    const d = this.data || {}, report = d.report || {}, activity = d.activity || {};
    const tools = report.tools || [];
    const part = (n: string, title: string, ...body: any[]) => el('section', {class: 'may-part may-p' + n},
      el('h2', {}, el('small', {}, n), title), ...body);
    const details = (title: string, value: unknown, open = false) => {
      let text = typeof value === 'string' ? value : JSON.stringify(value ?? null, null, 2);
      if (typeof value === 'string') { try { text = JSON.stringify(JSON.parse(value), null, 2); } catch {} }
      return el('details', {class: 'may-more', open}, el('summary', {}, title), el('pre', {}, text));
    };
    const outcome = (tool: any) => tool.error ? `Could not execute · ${tool.error}` :
      tool.signal != null ? `Terminated by signal ${tool.signal}` :
      tool.exit_code == null ? 'Not executed' : `Exit ${tool.exit_code}`;
    const profile = el('select', {id: 'analytics-profile', 'aria-label': 'Analyzer profile', disabled: this.busy,
      onchange: (event: any) => { this.profile = event.target.value; }},
      el('option', {value: 'quick'}, 'Quick · language checks'), el('option', {value: 'comprehensive'}, 'Comprehensive · all analyzers'));
    profile.value = this.profile;
    const actions = el('div', {class: 'report-actions'}, profile,
      el('button', {id: 'mayhem-search', class: 'start', disabled: this.busy, onclick: () => this.render(true)}, this.busy ? 'Working…' : 'Run analyzers'),
      el('button', {id: 'analytics-refresh', class: 'ghost', disabled: this.busy, onclick: () => this.render()}, 'Refresh saved results'));
    const measured = (type: string, field: string) => mayhemLizard(report, type)
      .filter(row => Number.isFinite(row.values[field])).sort((a, b) => b.values[field] - a.values[field]);
    const functions = measured('Function', 'CCN'), files = measured('File', 'NCSS');
    const complexity = el('div', {class: 'may-trend', role: 'img', 'aria-label': 'Functions by native Lizard CCN'});
    const sizes = el('div', {class: 'may-trend', role: 'img', 'aria-label': 'Files by native Lizard NCSS'});
    const hierarchy = el('details', {class: 'may-more'}, el('summary', {id: 'mayhem-command-tree'}, 'Explore magic-cli commands'));
    hierarchy.ontoggle = async () => {
      if (!hierarchy.open || hierarchy.querySelector('.may-tree')) return;
      const node = el('div', {class: 'may-tree', 'aria-label': 'Registered command groups'});
      hierarchy.append(node);
      try { await mayhemBundle('mayhem-charts'); if (node.isConnected) this.charts.push((window as any).magicMayhemTree(node, d.commands || [])); }
      catch (error) { node.textContent = String(error); }
    };
    const terminal = el('details', {class: 'may-more'}, el('summary', {}, 'Backstage terminal'));
    terminal.ontoggle = () => { if (terminal.open) { terminal.append(window.magicBackstageTerminal.mount()); window.dispatchEvent(new Event('backstage-visible')); } };
    const rows = activity.rows || [];
    const agents = activity.available ? [
      el('p', {class: 'muted'}, `${activity.scope} · ${activity.project_directory} · workspace ${activity.workspace_id}`),
      rows.length ? el('div', {class: 'may-agents'}, ...rows.map((row: any) => el('div', {class: 'may-agent'},
        el('b', {}, row.agent_name || 'Unknown agent name'), el('span', {}, row.agent_status || 'Unknown status'),
        el('small', {}, `Kind: ${row.agent_kind || 'Unknown'} · terminal: ${row.terminal_id || 'Unknown'}`),
        el('small', {}, `Workspace: ${row.workspace_name || 'Unknown'} (${row.workspace_id || 'Unknown'})`),
        el('small', {}, `Tab: ${row.tab_name || 'Unknown'} (${row.tab_id || 'Unknown'}) · pane: ${row.pane_name || 'Unknown'} (${row.pane_id || 'Unknown'})`),
        el('small', {}, row.cwd)))) : el('p', {}, 'No panes with a verified directory in this project/workspace.'),
      activity.unknown_directories ? el('p', {}, `${activity.unknown_directories} workspace panes have unknown directories and are excluded.`) : null,
    ] : [el('p', {}, activity.error || 'Herdr snapshot unavailable. Project/workspace activity is unknown.')];
    const events = (d.cli_events?.events || []).filter((row: any) => row.origin === 'CLI');
    const content = el('div', {class: 'may-summary may-dashboard'},
      el('header', {class: 'may-head'}, el('div', {}, el('small', {}, 'MAYHEM  /  NATIVE REPORTS'), el('h1', {}, 'Mayhem')), actions),
      this.error ? el('p', {role: 'alert'}, this.error) : null,
      part('01', 'Run evidence', el('p', {}, report.error || `Saved ${report.profile || 'Unknown profile'} run · ${mayhemTime(report.measured_at)}`),
        el('p', {}, `Source: ${report.source_root || d.root || 'Unknown'} · target: ${report.target || 'Unknown'}`),
        el('p', {}, `Revision: ${report.revision || 'Unknown'} · ${report.working_tree == null ? 'Working tree unknown' : report.working_tree ? 'Included uncommitted changes' : 'Recorded clean working tree'}`),
        el('p', {}, tools.length ? `${tools.filter((t: any) => t.exit_code === 0).length} commands returned 0; ${tools.filter((t: any) => t.exit_code != null && t.exit_code !== 0).length} returned nonzero; ${tools.filter((t: any) => t.exit_code == null).length} have no native exit code.` : 'No command outcomes recorded.'),
        el('p', {class: 'muted'}, 'These are command outcomes for the recorded scopes, not an application grade or coverage claim.')),
      part('02', 'Where are my agents?', ...agents),
      part('03', 'Native measurements', el('div', {class: 'may-plots'},
        el('figure', {}, el('figcaption', {}, 'Functions · Lizard CCN'), complexity),
        el('figure', {}, el('figcaption', {}, 'Files · Lizard NCSS'), sizes)),
        details('All Lizard function measurements', functions), details('All Lizard file measurements', files)),
      part('04', 'magic-cli', el('code', {}, 'magic-cli audit --quick · magic-cli audit · magic-cli audit --tool tsc'),
        el('p', {}, 'MCP: command_run mayhem.scan with quick=true or selected tools. Each report records its exact scope.'),
        events.length ? details('Observed CLI events in this session', events) : el('p', {}, 'No CLI events recorded in this session.'), hierarchy),
      part('05', 'Analyzer reports', ...tools.map((tool: any) => el('article', {class: 'may-tool'},
        el('strong', {}, tool.tool), el('span', {}, outcome(tool)), el('p', {}, `Scope: ${tool.scope}`),
        details('Exact invocation', {cwd: tool.cwd, executable: tool.executable, args: tool.args}),
        details('Complete stdout', tool.stdout, tool.exit_code !== 0 && Boolean(tool.stdout)), details('Complete stderr', tool.stderr, tool.exit_code !== 0 && Boolean(tool.stderr)), tool.native_report ? details('Native report file', tool.native_report.error || tool.native_report.contents) : null)),
        !tools.length ? el('p', {}, 'No analyzer report is loaded. Choose a profile and run it explicitly.') : null),
      details('Complete saved run', report),
      report.artifacts ? el('div', {}, el('code', {}, report.artifacts), el('button', {class: 'ghost', onclick: () => openPath(report.artifacts)}, 'Open raw artifacts')) : null,
      terminal);
    $('view-analytics').replaceChildren(content);
    const nav = $('mayhem-health');
    if (nav) { nav.textContent = this.busy ? 'Running' : report.measured_at ? 'Saved reports' : 'Not measured'; delete nav.parentElement.dataset.health; }
    void this.plot(complexity, functions.slice(0, 5).map(row => ({label: row.name, value: row.values.CCN})), 'CCN');
    void this.plot(sizes, files.slice(0, 5).map(row => ({label: row.name, value: row.values.NCSS})), 'NCSS');
  }
}
const mayhemView = new AnalyticsViewController();
async function renderAnalytics(scan = false) { return mayhemView.render(scan); }
