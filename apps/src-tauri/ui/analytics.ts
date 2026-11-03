// Mayhem: code, usage and tool reports. Saved measurements are read on entry;
// scanning is an explicit action, never a timer. Report pieces come from report.js.
const MAYHEM_SECTIONS = [['overview', 'Overview'], ['code', 'Code'], ['usage', 'Usage'], ['tools', 'Tools'],
  ['commands', 'Commands'], ['backstage', 'Backstage'], ['architecture', 'Architecture'], ['scatter', 'Complexity lab']];
// Existing Archify references, not live system models.
const MAYHEM_DIAGRAMS = [
  {id: 'magic-cli', file: 'magic-cli-architecture.html', title: 'Magic CLI & Studio architecture', source: 'Rust magic-cli · unified live Studio & headless engine routing', revision: null},
  {id: 'creator', file: 'creator-view.html', title: 'Creator’s loop', source: 'Existing Magic Studio creator workflow · saved 27 Oct 2026', revision: null},
];
const MAYHEM_SCAN_STATES = {measured: 'Explicit scan', partial: 'Partial scan', saved: 'Saved scan', stale: 'Saved scan · revision changed', worktree_unverified: 'Saved scan · worktree changes not verified'};

function mayhemTime(seconds) { return seconds == null ? 'time unavailable' : new Date(seconds * 1000).toLocaleString(); }
function mayhemMemory(kb) { return kb == null ? '—' : `${reportValue(Number(kb) / 1048576)} GB`; }
function mayhemMeasure(comp, key) { return comp.available === true ? comp[key] : null; }
function mayhemScanLabel(comp) { return comp.available ? MAYHEM_SCAN_STATES[comp.state] || 'Saved measurement' : 'Not measured'; }

function mayhemStats(...stats) { return el('div', {class: 'report-stats'}, ...stats.map((stat) => new ReportStat(stat).element)); }

function mayhemCodeStats(comp) {
  const total = Number(mayhemMeasure(comp, 'functions_analyzed')) || 0, over = Number(mayhemMeasure(comp, 'functions_over_ccn_15')) || 0;
  const top = comp.hotspots?.[0];
  return [
    {label: 'Functions over CCN 15', value: reportValue(mayhemMeasure(comp, 'functions_over_ccn_15')), signal: over ? 'needs-attention' : comp.available ? 'ready' : null,
      share: total ? over / total : null, detail: total ? `${reportValue(100 * over / total)}% of ${reportValue(total)} functions` : comp.reason || 'Scan code to measure.'},
    {label: 'Highest CCN', value: reportValue(mayhemMeasure(comp, 'max_ccn')), detail: top ? `${top.name} · ${top.file}:${top.start_line}` : '—'},
    {label: 'Source lines', value: reportValue(mayhemMeasure(comp, 'total_source_lines'), {compact: true}),
      detail: comp.available ? `${reportValue(comp.files_analyzed)} files with functions` : '—'},
  ];
}

function mayhemUsageStats(usage) {
  const totals = usage.available ? usage.totals || {} : {};
  const active = (usage.agents || []).filter((agent) => agent.today_tokens > 0).length;
  return [
    {label: 'Tokens today', value: reportValue(totals.today_tokens, {compact: true}),
      detail: usage.available ? `${reportValue(totals.today_prompts)} prompts · ${active} agents active` : 'Usage records unavailable.'},
    {label: 'Tokens recorded', value: reportValue(totals.total_tokens, {compact: true}),
      detail: totals.total_tokens ? `${reportValue(100 * totals.cache_read_tokens / totals.total_tokens)}% cache reads` : '—'},
  ];
}

function mayhemToolStat(tools = []) {
  const found = tools.filter((tool) => tool.available).length;
  return {label: 'Tools found', value: tools.length ? `${found} / ${tools.length}` : '—',
    signal: tools.length ? (found === tools.length ? 'ready' : 'needs-attention') : 'idle', detail: 'Executable on PATH · not a run'};
}

function mayhemAuditSignal(audit: any): string {
  if (!audit.available) return 'idle';
  return Number(audit.failed) ? 'needs-attention' : 'ready';
}

function mayhemAuditStat(audit: any = {}) {
  const total = Number(audit.total) || 0, passed = Number(audit.passed) || 0;
  const findings = Number(audit.total_findings) || 0;
  return {label: 'Audit checks', value: total ? `${passed} / ${total}` : '—',
    signal: mayhemAuditSignal(audit), share: total ? passed / total : null,
    detail: audit.available ? `${findings} findings · rev ${audit.source_revision || '—'}` : 'Run magic-cli audit to populate'};
}

function mayhemAuditChecks(audit: any = {}) {
  return new ReportTable({rows: audit.tools || [], empty: 'Run magic-cli audit or magic-cli review to record check numbers.', columns: [
    {label: 'Check', value: (row) => el('span', {class: 'report-name'}, window.companionBoxes.dot(row.ok ? 'ready' : 'needs-attention'), row.label || row.tool)},
    {label: 'Status', value: (row) => row.ok ? 'PASS' : 'FAIL'},
    {label: 'Findings', value: (row) => row.findings ?? 0, numeric: true},
    {label: 'Exit', value: (row) => row.exit_code ?? '—', numeric: true},
    {label: 'Summary', value: (row) => el('code', {}, row.note || (row.ok ? 'ok' : 'failed'))},
  ]}).element;
}

function mayhemHistory(comp: any = {}) {
  const rows = [...(comp.history || [])].reverse().slice(0, 10);
  return new ReportTable({rows, empty: 'Run magic-cli audit to append revision snapshots.', columns: [
    {label: 'When', value: (row) => mayhemTime(row.measured_at_unix_secs)},
    {label: 'Revision', value: (row) => el('code', {}, row.source_revision || '—')},
    {label: 'Functions', value: (row) => row.functions_analyzed, numeric: true},
    {label: 'Over CCN 15', value: (row) => row.functions_over_ccn_15, numeric: true},
    {label: 'Max CCN', value: (row) => row.max_ccn, numeric: true},
    {label: 'Source lines', value: (row) => row.total_source_lines, numeric: true, compact: true},
    {label: 'Long files', value: (row) => row.long_files_count ?? null, numeric: true},
  ]}).element;
}

const MAYHEM_LANG_LABELS: Record<string, string> = {
  rust: 'Rust', typescript: 'TypeScript', tsx: 'TSX · Remotion', javascript: 'JavaScript',
  css: 'CSS', html: 'HTML · Shell', markdown: 'Markdown', python: 'Python',
};

function inferMayhemLang(file = '', explicit = '') {
  if (explicit) return explicit;
  const ext = file.split('.').pop() || '';
  const map: Record<string, string> = {rs: 'rust', ts: 'typescript', tsx: 'tsx', js: 'javascript', mjs: 'javascript', css: 'css', html: 'html', md: 'markdown', py: 'python'};
  return map[ext] || 'other';
}

function mayhemLangBadge(file = '', explicit = '', customLabel = '') {
  const lang = inferMayhemLang(file, explicit);
  const label = customLabel || MAYHEM_LANG_LABELS[lang] || lang;
  return el('span', {class: 'report-lang', 'data-lang': lang}, label);
}

function mayhemOpenFile(comp: any, relPath: string, label: string) {
  if (!relPath) return '—';
  const fullPath = comp?.source_root ? `${comp.source_root}/${relPath}` : relPath;
  return el('button', {class: 'file', title: `Open ${fullPath}`, 'aria-label': `Open ${relPath}`, onclick: () => openPath(fullPath)},
    el('code', {}, label));
}

function mayhemHotspots(comp, limit) {
  return new ReportTable({rows: (comp.hotspots || []).slice(0, limit).map((fn) => ({...fn, lang: inferMayhemLang(fn.file, fn.lang)})), empty: comp.reason || 'No saved scan. Scan code to measure.', columns: [
    {label: 'Lang', value: (fn) => mayhemLangBadge(fn.file, fn.lang)},
    {label: 'Function', value: (fn) => fn.name},
    {label: 'Where', value: (fn) => mayhemOpenFile(comp, fn.file, `${fn.file.split('/').pop()}:${fn.start_line}`), title: (fn) => `${fn.file}:${fn.start_line}–${fn.end_line}`},
    {label: 'CCN', value: (fn) => fn.ccn, numeric: true, bar: true},
    {label: 'NLOC', value: (fn) => fn.nloc, numeric: true},
  ]}).element;
}

function mayhemLanguageBreakdown(comp) {
  return new ReportTable({rows: comp.languages || [], empty: 'Run Scan code to build the per-language breakdown.', columns: [
    {label: 'Language', value: (row) => mayhemLangBadge('', row.lang, row.label)},
    {label: 'Lines', value: (row) => row.lines, numeric: true, bar: true},
    {label: 'Files', value: (row) => row.files, numeric: true},
    {label: 'Over CCN 15', value: (row) => row.over_15, numeric: true},
    {label: 'Max CCN', value: (row) => row.max_ccn || '—', numeric: true},
    {label: 'Biggest offender', value: (row) => row.top_file ? mayhemOpenFile(comp, row.top_file, `${row.top_file.split('/').pop()} (${reportValue(row.top_file_lines)} lines)`) : '—', title: (row) => row.top_file || null},
  ]}).element;
}

function mayhemAgents(usage, todayOnly) {
  const rows = (usage.agents || []).filter((agent) => (todayOnly ? agent.today_tokens : agent.total_tokens) > 0);
  const limit = (agent) => agent.limits?.[0];
  return new ReportTable({rows: rows, empty: usage.available ? 'No recorded usage.' : 'Usage records unavailable.', columns: [
    {label: 'Agent', value: (agent) => agent.name || agent.id},
    {label: 'Today', value: (agent) => agent.today_tokens, numeric: true, compact: true, bar: true},
    ...(todayOnly ? [] : [
      {label: 'Recorded', value: (agent) => agent.total_tokens, numeric: true, compact: true},
      {label: 'Prompts', value: (agent) => agent.total_prompts, numeric: true},
      {label: 'Top model', value: (agent) => agent.models?.[0]?.model || '—'}]),
    {label: 'Limit used', value: (agent) => limit(agent)?.percent ?? null, numeric: true,
      text: (agent) => limit(agent) ? `${reportValue(100 * limit(agent).percent)}%` : '—', title: (agent) => limit(agent)?.label ?? null},
  ]}).element;
}

function mayhemCoverage(comp) {
  if (!comp.available) return null;
  const failed = comp.scan_failures || [];
  const dirty = comp.source_dirty === true ? 'worktree had changes' : comp.source_dirty === false ? 'clean worktree' : 'worktree unknown';
  return `${mayhemScanLabel(comp)} · ${mayhemTime(comp.measured_at_unix_secs)} · ${dirty}${failed.length ? ` · not measured: ${failed.join(', ')}` : ''} · Lizard ${comp.tool_version || ''}`;
}

function mayhemHealthWarnings(d: any): string[] {
  const {complexity: comp = {}, tools = [], runtime_errors: runtime = {}, audit = {}} = d || {};
  const pairs: [number, string][] = [
    [tools.filter(tool => tool.available === false).length, 'tools not found'],
    [(comp.scan_failures || []).length, 'scan targets failed'],
    [Number(mayhemMeasure(comp, 'functions_over_ccn_15')) || 0, 'functions over CCN 15 in saved scan'],
    [Number(audit.failed) || 0, 'audit checks failed in saved review'],
    [Number(runtime.retained_error_count) || 0, 'recorded runtime errors'],
  ];
  const warnings = pairs.filter(([n]) => n > 0).map(([n, text]) => `${n} ${text}`);
  if (comp.last_error || comp.cache_error) warnings.push('scan error recorded');
  return warnings;
}
type MayhemChartRow = {label: string; value: number};
type MayhemScatterRow = {label: string; ccn: number; nloc: number};
type MayhemResource = {dispose(): void};
type MayhemJevItem = {id: string; source: string; evidence: unknown; choice: string; confidence: number | null; needs_review: boolean; error?: string};
type MayhemJevReport = {available?: boolean; state?: string; measured_at_unix_secs?: number; rubric?: Record<string, unknown>;
  counts?: {total: number; excluded: number; truncated: number; failed: number; by_label: Record<string, number>};
  items?: MayhemJevItem[]; error?: string};
type MayhemChartSource = {complexity?: {available?: boolean; languages?: {lang: string; label?: string; lines?: number}[];
  history?: {measured_at_unix_secs?: number; max_ccn?: number}[]};
  usage?: {available?: boolean; agents?: {name?: string; id?: string; today_tokens?: number}[]};
  audit?: {available?: boolean; tools?: {label?: string; tool?: string; findings?: number}[]}};

function mayhemNumber(value: unknown): value is number { return typeof value === 'number' && Number.isFinite(value) && value >= 0; }

function mayhemChartSeries(d: MayhemChartSource) {
  const comp = d.complexity || {}, usage = d.usage || {}, audit = d.audit || {};
  return {
    languages: comp.available ? (comp.languages || []).filter(row => mayhemNumber(row.lines))
      .map(row => ({label: row.label || MAYHEM_LANG_LABELS[row.lang] || row.lang, value: row.lines})) : [],
    usage: usage.available ? (usage.agents || []).filter(row => mayhemNumber(row.today_tokens))
      .map(row => ({label: row.name || row.id || 'Unnamed agent', value: row.today_tokens})) : [],
    history: comp.available ? (comp.history || []).filter(row => mayhemNumber(row.measured_at_unix_secs) && mayhemNumber(row.max_ccn))
      .map(row => ({label: mayhemTime(row.measured_at_unix_secs), value: row.max_ccn})) : [],
    checks: audit.available ? (audit.tools || []).filter(row => mayhemNumber(row.findings))
      .map(row => ({label: row.label || row.tool || 'Unnamed check', value: row.findings})) : [],
  };
}

function mayhemScatterRows(comp: {available?: boolean; hotspots?: {name?: string; file?: string; start_line?: number; ccn?: number; nloc?: number}[]}): MayhemScatterRow[] {
  return comp.available ? (comp.hotspots || []).filter(row => mayhemNumber(row.ccn) && mayhemNumber(row.nloc))
    .map(row => ({label: `${row.name || 'Unnamed'} · ${row.file || '?'}:${row.start_line ?? '?'}`, ccn: row.ccn, nloc: row.nloc})) : [];
}

const mayhemBundles = new Map<string, Promise<void>>();
function mayhemBundle(file: string): Promise<void> {
  // This WebKit UI targets ES2023, before Promise.withResolvers.
  if (!mayhemBundles.has(file)) mayhemBundles.set(file, new Promise((resolve, reject) => {
    const script = document.createElement('script');
    script.src = `vendor/${file}.bundle.js`;
    script.onload = () => resolve();
    script.onerror = () => { script.remove(); mayhemBundles.delete(file); reject(new Error(`Could not load ${file}. Rebuild frontend bundles.`)); };
    document.head.append(script);
  }));
  return mayhemBundles.get(file);
}

function mayhemEvidence(title: string, value: unknown) {
  return el('details', {class: 'mayhem-evidence'}, el('summary', {}, title),
    el('pre', {}, JSON.stringify(value ?? null, null, 2)));
}

function mayhemChart(title: string, rows: MayhemChartRow[], note: string, unit: string, tone: string, kind = 'bar') {
  const chart = el('div', {class: 'mayhem-chart', role: 'img', 'aria-label': `${title} · ${unit}`});
  chart.style.setProperty('--mayhem-chart-rows', String(kind === 'bar' ? rows.length : 0));
  const fallback = el('details', {class: 'mayhem-evidence'}, el('summary', {}, `Exact values · ${rows.length} measurements`),
    new ReportTable({rows, columns: [{label: 'Source', value: row => row.label}, {label: unit, value: row => row.value, numeric: true}]}).element);
  const body = rows.length ? chart : el('p', {class: 'mayhem-empty'}, 'No saved measurements for this chart. Unknown is not zero.');
  if (rows.length) {
    const epoch = mayhemView.epoch;
    queueMicrotask(async () => {
      try {
        await mayhemBundle('mayhem-charts');
        if (epoch !== mayhemView.epoch || !chart.isConnected || !mayhemView.visible()) return;
        const api = window['magicMayhemCharts'] as {mount(node: HTMLElement, rows: MayhemChartRow[], kind: string, color: string, unit: string): MayhemResource};
        const color = getComputedStyle(chart).getPropertyValue(tone).trim();
        mayhemView.resources.push(api.mount(chart, rows, kind, color, unit));
      } catch (error) {
        if (!chart.isConnected || epoch !== mayhemView.epoch) return;
        chart.classList.add('mayhem-empty');
        chart.textContent = `Chart unavailable: ${String(error)}. Exact saved values remain below.`;
        fallback.open = true;
      }
    });
  }
  return new ReportSection({title, note}, body, fallback).element;
}

function mayhemJevTable(items: MayhemJevItem[]) {
  return new ReportTable({rows: items, empty: 'No recorded items.', columns: [
    {label: 'Unit', value: row => el('code', {}, row.id)},
    {label: 'Source', value: row => row.source},
    {label: 'Jev choice', value: row => row.choice || 'No choice'},
    {label: 'Probability', value: row => row.confidence, text: row => mayhemNumber(row.confidence) ? `${reportValue(row.confidence * 100)}%` : 'Unknown', numeric: true},
    {label: 'Review', value: row => row.error || (row.needs_review ? 'Needs human review' : 'Not escalated')},
    {label: 'Evidence', value: row => mayhemEvidence('Inspect native evidence', row.evidence)},
  ]}).element;
}

function mayhemJev(report: MayhemJevReport = {}) {
  const hasReport = Boolean(report.available || report.measured_at_unix_secs);
  const counts = hasReport ? report.counts : undefined, items = report.items || [], running = mayhemView.judging;
  const escalation = items.filter(item => item.needs_review || item.error);
  const partition = report.rubric?.partition;
  const cap = partition && typeof partition === 'object' && 'max_batch_items' in partition && mayhemNumber(partition.max_batch_items) ? partition.max_batch_items : null;
  const step = (n: string, title: string, summary: string, ...body: HTMLElement[]) =>
    el('section', {class: 'mayhem-step'}, el('span', {class: 'mayhem-step-number'}, n),
      el('div', {}, el('h3', {}, title), el('p', {}, summary), ...body));
  const status = running ? 'Running Jev on saved evidence. No staged progress is inferred.'
    : mayhemView.judgeError ? `Run failed: ${mayhemView.judgeError}. Previous report remains below.`
    : `${report.state || 'Not run'}${hasReport ? ` · saved ${mayhemTime(report.measured_at_unix_secs)}` : ''}${report.error ? ` · ${report.error}` : ''}`;
  const button = el('button', {class: 'start', disabled: running || mayhemView.loading,
    'data-commands': 'app.analytics', onclick: () => mayhemView.runJev(),
    title: 'Run Jev once on saved native evidence only; does not scan code or execute audit tools'}, running ? 'Running Jev…' : 'Run Jev');
  return new ReportSection({title: 'From measurements to decisions', note: 'Native tools measure. Jev judges. A person approves the work.', actions: [button]},
    el('p', {class: 'mayhem-jev-status', role: 'status', 'aria-live': 'polite'}, status),
    el('div', {class: 'mayhem-steps'},
      step('01', 'Decide', 'Fixed choices: actionable · observe · insufficient. Rubric is frozen by the backend for each run.',
        mayhemEvidence('Frozen rubric · exact recorded contract', report.rubric || 'Run Jev to record the backend rubric.')),
      step('02', 'Partition', counts ? `${counts.total} candidate units · ${counts.excluded} excluded · ${counts.truncated} evidence clipped · ${items.length} submitted${cap === null ? '' : ` · ${cap} per batch`}.`
        : `Existing native evidence only. One evidence record per unit${cap === null ? '; backend batch size not yet available' : `; ${cap} per batch`}. Exclusion policy below.`,
        mayhemEvidence('Unit, exclusion and cap policy', partition || 'Backend partition policy unavailable.')),
      step('03', 'Judge', counts ? `${items.length} item results · ${counts.failed} failed. Probabilities come from Jev, not a score invented by Mayhem.`
        : 'No judgment yet. Run Jev explicitly; browsing this page never starts inference.',
        el('details', {class: 'mayhem-evidence'}, el('summary', {}, 'Batch results and native evidence'), mayhemJevTable(items))),
      step('04', 'Escalate', hasReport ? `${escalation.length} items require review or have a recorded error. Nothing is automatically repaired.`
        : 'Low-confidence choices and errors remain visible for human review.',
        el('details', {class: 'mayhem-evidence'}, el('summary', {}, 'Escalation queue'), mayhemJevTable(escalation))),
      step('05', 'Report', counts ? Object.entries(counts.by_label || {}).map(([label, count]) => `${label}: ${count}`).join(' · ') || 'No successful choices recorded.'
        : 'No saved Jev summary. Native measurements below remain available.',
        mayhemEvidence('Full saved Jev report', report))),
  ).element;
}

class AnalyticsViewController {
  commandFilter = '';
  data: any = null;
  diagram = 'magic-cli';
  error: string | null = null;
  loading = false;
  section = 'overview';
  judging = false;
  judgeError: string | null = null;
  epoch = 0;
  resources: MayhemResource[] = [];
  webgl = false;

  visible() { return !document.hidden && $('view-analytics')?.classList.contains('active'); }

  disposeVisuals() {
    this.epoch++;
    for (const resource of this.resources.splice(0)) resource.dispose();
  }

  async runJev() {
    if (this.judging || this.loading) return;
    this.judging = true;
    this.judgeError = null;
    $('analytics-refresh').disabled = true;
    $('analytics-scan').disabled = true;
    this.renderSurface();
    try {
      const result = await invoke('engine_command', {id: 'app.analytics', params: {judge: true}});
      if (!result?.jev || typeof result.jev !== 'object') throw new Error('Backend returned no Jev report.');
      this.data = {...(this.data || {}), jev: result.jev};
    } catch (error) {
      this.judgeError = String(error);
    } finally {
      this.judging = false;
      $('analytics-refresh').disabled = false;
      $('analytics-scan').disabled = false;
      this.renderSurface();
    }
  }

  async render(scan = false) {
    if (!$('view-analytics') || this.loading || this.judging) return;
    this.mount();
    this.loading = true;
    this.error = null;
    $('analytics-refresh').disabled = true;
    $('analytics-scan').disabled = true;
    this.renderSurface();
    $('analytics-message').textContent = scan ? 'Running Lizard on local source…' : 'Reading saved scan and recorded usage…';
    try {
      this.data = await invoke('engine_command', {id: 'app.analytics', params: {scan}});
    } catch (error) {
      this.error = String(error);
    } finally {
      this.loading = false;
      $('analytics-refresh').disabled = false;
      $('analytics-scan').disabled = false;
      this.healthIndicator();
      this.renderSurface();
    }
  }

  mount() {
    if ($('analytics-layout')) return;
    const tabs = el('nav', {class: 'segmented', id: 'mayhem-sections', role: 'tablist', 'aria-label': 'Mayhem sections',
      onkeydown: (event: KeyboardEvent) => {
        const index = MAYHEM_SECTIONS.findIndex(([id]) => id === this.section);
        const next = event.key === 'ArrowRight' ? (index + 1) % MAYHEM_SECTIONS.length
          : event.key === 'ArrowLeft' ? (index + MAYHEM_SECTIONS.length - 1) % MAYHEM_SECTIONS.length
          : event.key === 'Home' ? 0 : event.key === 'End' ? MAYHEM_SECTIONS.length - 1 : -1;
        if (next < 0) return;
        event.preventDefault();
        this.section = MAYHEM_SECTIONS[next][0];
        this.renderSurface();
        $(`analytics-nav-${this.section}`).focus();
      }},
      ...MAYHEM_SECTIONS.map(([id, label]) => el('button', {id: `analytics-nav-${id}`, role: 'tab',
        onclick: () => { this.section = id; this.renderSurface(); }}, label)));
    const actions = el('div', {class: 'report-actions'},
      el('button', {id: 'analytics-refresh', class: 'ghost', title: 'Read the saved scan and recorded usage without running Lizard', 'data-commands': 'app.analytics', onclick: () => this.render(false)}, 'Refresh'),
      el('button', {id: 'analytics-scan', class: 'start', title: 'Measure local Rust, Python and shell JavaScript with Lizard now', 'data-commands': 'app.analytics', onclick: () => this.render(true)}, 'Scan code'));
    const head = el('header', {class: 'report-head'},
      el('div', {class: 'report-title'}, el('h1', {}, 'Mayhem'),
        el('p', {id: 'analytics-message', class: 'panel-status', role: 'status'}), actions), tabs);
    const surface = el('div', {id: 'analytics-surface', class: 'report-surface'});
    const backstage = el('div', {class: 'report-surface', id: 'analytics-backstage'}, window.magicBackstageTerminal.mount());
    $('view-analytics').replaceChildren(el('div', {class: 'report-page', id: 'analytics-layout'},
      head, el('div', {class: 'report-scroll'}, surface, backstage)));
    const visibility = () => {
      this.disposeVisuals();
      this.webgl = false;
      if (this.visible()) this.renderSurface();
    };
    new MutationObserver(visibility).observe($('view-analytics'), {attributes: true, attributeFilter: ['class', 'hidden', 'style']});
    document.addEventListener('visibilitychange', visibility);
    window.addEventListener('pagehide', () => { this.disposeVisuals(); this.webgl = false; });
  }

  renderSurface() {
    const surface = $('analytics-surface');
    if (!surface) return;
    this.disposeVisuals();
    if (this.section !== 'scatter') this.webgl = false;
    const d = this.data || {};
    const backstage = this.section === 'backstage';
    $('analytics-backstage').hidden = !backstage;
    surface.hidden = backstage;
    if (backstage) window.dispatchEvent(new Event('backstage-visible'));
    for (const [id] of MAYHEM_SECTIONS) {
      $(`analytics-nav-${id}`).setAttribute('aria-selected', String(id === this.section));
      $(`analytics-nav-${id}`).tabIndex = id === this.section ? 0 : -1;
    }
    $('analytics-message').textContent = this.error
      ? `Could not refresh: ${this.error}. Previous results, if any, remain below.`
      : this.loading ? 'Reading native measurements…'
      : `Read ${mayhemTime(d.sampled_at_unix_secs)} · no background scans or polling`;
    surface.replaceChildren(...MAYHEM_RENDER[this.section](d));
  }

  healthIndicator() {
    const warnings = mayhemHealthWarnings(this.data);
    const button = document.getElementById('tab-analytics');
    const stale = Boolean(this.error);
    button.dataset.health = warnings.length ? 'warning' : 'unknown';
    document.getElementById('mayhem-health').textContent = warnings.length ? `${stale ? 'Previous · ' : ''}${warnings.length} signal${warnings.length === 1 ? '' : 's'}` : 'Unknown';
    button.title = `Mayhem · check here first. ${warnings.length ? warnings.join('; ') : 'Overall health unknown; no connected audit result.'}${stale ? ' Refresh failed; previous observations retained.' : ''} Executable presence is not runtime health.`;
  }
}

const mayhemView = new AnalyticsViewController();

function mayhemOverview(d) {
  const comp = d.complexity || {}, usage = d.usage || {}, audit = d.audit || {};
  const series = mayhemChartSeries(d);
  const largestFirst = (rows: MayhemChartRow[]) => [...rows].sort((a, b) => b.value - a.value);
  return [
    el('div', {class: 'mayhem-overview-intro'},
      el('div', {}, el('span', {class: 'mayhem-eyebrow'}, 'Engineering observatory'),
        el('h2', {}, 'Measure first. Choose what matters.'),
        el('p', {}, 'Saved source measurements, recorded local usage and explicit Jev decisions. Missing evidence stays unknown.')),
      el('span', {class: 'mayhem-source-badge'}, 'SVG charts · WebGL off')),
    mayhemStats(...mayhemCodeStats(comp).slice(0, 2), mayhemAuditStat(audit), mayhemUsageStats(usage)[0], mayhemToolStat(d.tools)),
    mayhemJev(d.jev),
    el('div', {class: 'mayhem-chart-grid'},
      mayhemChart('Source footprint', largestFirst(series.languages), mayhemCoverage(comp) || 'No saved language measurements.', 'source lines', '--accent'),
      mayhemChart('Recorded usage today', largestFirst(series.usage), 'Local agent records · tokens, not billing · missing agents omitted', 'tokens', '--motion-blue'),
      mayhemChart('Peak complexity over time', series.history, 'Saved revision snapshots · max function CCN, not an overall health score', 'max CCN', '--depth-gold', 'line'),
      mayhemChart('Saved audit finding counts', largestFirst(series.checks), 'Legacy audit report counts, not independently verified native findings. See architecture review.', 'reported findings', '--pose-purple')),
    el('details', {class: 'mayhem-evidence mayhem-raw'},
      el('summary', {}, 'Inspect saved native evidence · detailed sections remain available above'),
      new ReportSection({title: 'Audit & review checks', note: 'Saved report only. Executable presence and exit status are not workflow health.'}, mayhemAuditChecks(audit)).element,
      mayhemRuntimeErrors(d.runtime_errors),
      new ReportSection({title: 'Language evidence', note: mayhemCoverage(comp)}, mayhemLanguageBreakdown(comp)).element,
      new ReportSection({title: 'Saved hotspots', note: 'Retained hotspot subset, not a complete function distribution.'}, mayhemHotspots(comp, 25)).element,
      mayhemEvidence('Complete analytics response', d)),
  ];
}

function mayhemRuntimeErrors(report: any) {
  return new ReportSection({title: 'Recorded runtime errors', note: report?.coverage || 'Runtime error report unavailable.'},
    new ReportTable({rows: report?.errors || [], empty: 'No retained errors for this project. Workflows remain unassessed.', columns: [
      {label: 'When', value: row => mayhemTime(Number(row.unix_micros) / 1000000)},
      {label: 'Operation', value: row => row.command || row.stage},
      {label: 'Origin', value: row => row.origin || 'Unspecified'},
      {label: 'Error', value: row => row.error || 'Failure recorded without details', title: row => [row.error, ...(row.causes || [])].filter(Boolean).join('\n')},
      {label: 'Request', value: row => row.request_id || '—'},
    ]}).element).element;
}

function mayhemFileTable(comp, rows) {
  return new ReportTable({rows: (rows || []).map((f) => ({...f, lang: inferMayhemLang(f.file, f.lang)})), empty: 'No saved scan.', columns: [
    {label: 'Lang', value: (file) => mayhemLangBadge(file.file, file.lang)},
    {label: 'File', value: (file) => mayhemOpenFile(comp, file.file, file.file.split('/').pop()), title: (file) => file.file},
    {label: 'Lines', value: (file) => file.lines, numeric: true, bar: true},
    {label: 'Total CCN', value: (file) => file.ccn, numeric: true, bar: true},
    {label: 'Max CCN', value: (file) => file.max_ccn, numeric: true},
    {label: 'Functions', value: (file) => file.functions, numeric: true},
  ]}).element;
}

function mayhemCode(d) {
  const comp = d.complexity || {};
  const provenance = el('details', {class: 'report-provenance'}, el('summary', {}, 'Measurement source and coverage'),
    el('code', {}, `${comp.source_root || 'Source root unavailable'}\nrevision ${comp.source_revision || 'unavailable'}\n${comp.tool_path || 'Tool path unavailable'}\nTargets: ${(comp.targets || []).join(', ')}`));
  return [
    mayhemStats(...mayhemCodeStats(comp)),
    new ReportSection({title: 'By language & biggest offenders', note: 'Physical lines (reading cost) and cyclomatic complexity (change cost) across Rust, TypeScript, CSS, Markdown, HTML, TSX, and Python.'}, mayhemLanguageBreakdown(comp)).element,
    new ReportSection({title: 'Function hotspots', note: 'CCN and NLOC are Lizard measurements; CCN 15 is this repository’s limit.'}, mayhemHotspots(comp, 25)).element,
    el('div', {class: 'report-columns'},
      new ReportSection({title: 'Largest files by line count', note: '2,000 lines is the long-file threshold.'}, mayhemFileTable(comp, comp.longest_files)).element,
      new ReportSection({title: 'Most complex files by total CCN', note: mayhemCoverage(comp)}, mayhemFileTable(comp, comp.largest_ccn_files), provenance).element),
    new ReportSection({title: 'Revision complexity history', note: 'Populated by magic-cli audit / magic-cli review and explicit scans.'}, mayhemHistory(comp)).element,
  ];
}

function mayhemUsage(d) {
  const usage = d.usage || {}, project = d.project || {};
  return [
    mayhemStats(...mayhemUsageStats(usage),
      {label: 'This project’s chat', value: project.project ? reportValue(project.estimated_tokens, {compact: true}) : '—',
        detail: project.project ? `${reportValue(project.total_turns)} turns · estimate: characters ÷ 4` : 'No project selected.'}),
    new ReportSection({title: 'Agents', note: 'Omarchy local usage records · not provider billing'}, mayhemAgents(usage, false)).element,
  ];
}

function mayhemTools(d) {
  const host = d.host || {}, audit = d.audit || {};
  const tools = new ReportTable({rows: d.tools || [], empty: 'Refresh to check tools.', columns: [
    {label: 'Tool', value: (tool) => el('span', {class: 'report-name'}, window.companionBoxes.dot(tool.available ? 'ready' : 'needs-attention'), tool.tool)},
    {label: 'Resolved', value: (tool) => el('code', {}, tool.available ? tool.resolved : 'not found')},
  ]}).element;
  const used = host.mem_total_kb ? host.mem_used_kb / host.mem_total_kb : null;
  return [
    mayhemStats(mayhemAuditStat(audit), mayhemToolStat(d.tools),
      {label: 'Studio process', value: mayhemMemory(host.self_rss_kb), detail: `PID ${host.pid ?? '—'} · resident memory`},
      {label: 'System memory', value: mayhemMemory(host.mem_used_kb), share: used, detail: `of ${mayhemMemory(host.mem_total_kb)} · load ${host.load || '—'}`}),
    new ReportSection({title: 'Saved audit & review checks', note: 'Recorded by magic-cli audit / magic-cli review into target/analytics/audit.json.'}, mayhemAuditChecks(audit)).element,
    new ReportSection({title: 'Production tools', note: 'Found on PATH is not a successful run. Read from /proc and PATH.'}, tools).element,
  ];
}

function mayhemCommands(d) {
  const catalog = d.commands;
  if (!catalog?.commands) return [el('p', {class: 'panel-empty', 'data-look': 'inline'}, 'Command metadata unavailable. Refresh to read it.')];
  const input = el('input', {id: 'analytics-command-search', class: 'field report-search', type: 'search', placeholder: 'Find a command or feature',
    'aria-label': 'Find current command', value: mayhemView.commandFilter,
    oninput: (event) => { mayhemView.commandFilter = event.target.value; mayhemCommandRows(catalog.commands); }});
  const section = new ReportSection({title: `Commands · ${catalog.commands.length}`, note: 'From the shared engine command.list. Run with magic-cli exec ID key=value.'},
    input, el('div', {id: 'analytics-command-list', class: 'report-body'}));
  queueMicrotask(() => mayhemCommandRows(catalog.commands));
  return [section.element];
}

function mayhemCommandRows(commands) {
  const root = $('analytics-command-list');
  if (!root) return;
  const filter = mayhemView.commandFilter.toLowerCase();
  const groups = new Map();
  for (const command of commands.filter((c) => `${c.id} ${c.label} ${c.params}`.toLowerCase().includes(filter))) {
    const key = command.menu?.[0] || command.id.split('.')[0];
    groups.set(key, [...(groups.get(key) || []), command]);
  }
  root.replaceChildren(...[...groups].map(([name, items]) => el('details', {class: 'report-group', open: Boolean(filter)},
    el('summary', {}, `${name} · ${items.length}`),
    new ReportTable({rows: items, columns: [
      {label: 'Command', value: (c) => c.label},
      {label: 'ID', value: (c) => el('code', {title: c.params}, c.id)},
      {label: 'Shortcut', value: (c) => c.shortcut || '—'},
    ]}).element)));
}

function mayhemArchitecture() {
  const selected = MAYHEM_DIAGRAMS.find((item) => item.id === mayhemView.diagram) || MAYHEM_DIAGRAMS[0];
  const picker = el('select', {id: 'analytics-diagram-select', class: 'field', 'aria-label': 'Architecture reference',
    onchange: (event) => { mayhemView.diagram = event.target.value; mayhemView.renderSurface(); }},
    ...MAYHEM_DIAGRAMS.map((item) => el('option', {value: item.id}, item.title)));
  picker.value = selected.id;
  return [
    el('details', {class: 'mayhem-evidence mayhem-architecture-review'},
      el('summary', {}, 'Audit architecture review · source review, not live telemetry'),
      el('p', {}, 'Recommendation: native structured outputs → saved evidence with per-tool provenance → explicit Jev judgment → reviewed actions. Simplify the existing audit path rather than add bespoke collectors.'),
      el('ul', {},
        el('li', {}, el('code', {}, 'audit.rs::build_native_report'), ' assigns the Lizard file count to every tool’s filesAnalyzed and converts a failed check to one generic finding. Those fields are not native tool measurements.'),
        el('li', {}, el('code', {}, 'record_mayhem_audit'), ' can rerun Lizard while recording an audit, coupling report persistence to another scan.'),
        el('li', {}, el('code', {}, 'complexity::read_history'), ' now keeps saved history reads side-effect free. Only explicit collection persists new history.')),
      el('p', {}, 'This is a static architecture review, not a workflow-health assessment. The audit CLI reporting rewrite remains a redesign recommendation.')),
    new ReportSection({title: 'How the work fits together', actions: [picker],
      note: `Saved Archify reference, not a live model · ${selected.source}${selected.revision ? ` · revision ${selected.revision}` : ''}`},
    el('iframe', {id: 'analytics-diagram-frame', class: 'report-frame', title: selected.title,
      src: `analytics-diagrams/${selected.file}?theme=dark&embed=1`, sandbox: 'allow-scripts'})).element,
  ];
}

function mayhemScatter(d) {
  const rows = mayhemScatterRows(d.complexity || {});
  const canvas = el('div', {class: 'mayhem-scatter', 'aria-label': 'Optional WebGL complexity scatter'});
  const button = el('button', {class: 'ghost', 'aria-pressed': String(mayhemView.webgl), disabled: !rows.length,
    onclick: () => { mayhemView.webgl = !mayhemView.webgl; mayhemView.renderSurface(); }},
    mayhemView.webgl ? 'Disable WebGL · release GPU' : 'Enable WebGL scatter');
  if (mayhemView.webgl && rows.length) {
    canvas.textContent = 'Loading optional renderer…';
    const epoch = mayhemView.epoch;
    queueMicrotask(async () => {
      try {
        await mayhemBundle('mayhem-scatter');
        if (epoch !== mayhemView.epoch || mayhemView.section !== 'scatter' || !mayhemView.webgl || !mayhemView.visible() || !canvas.isConnected) return;
        canvas.replaceChildren();
        const api = window['magicMayhemScatter'] as {mount(node: HTMLElement, rows: MayhemScatterRow[]): MayhemResource};
        mayhemView.resources.push(api.mount(canvas, rows));
      } catch (error) {
        if (!canvas.isConnected || epoch !== mayhemView.epoch) return;
        canvas.textContent = `WebGL unavailable: ${String(error)}. Exact saved evidence is available below.`;
      }
    });
  } else {
    canvas.append(el('p', {class: 'mayhem-empty'}, rows.length
      ? 'GPU is off. Enable explicitly to plot saved hotspots by NLOC and CCN. No animation loop.'
      : 'No saved hotspots with both CCN and NLOC. Run an explicit code scan to collect measurements.'));
  }
  return [new ReportSection({title: 'Complexity lab', note: 'Optional static WebGL view. Only retained hotspots are plotted, not a complete function distribution. Leaving or hiding this view releases GPU resources; returning requires opt-in again.', actions: [button]},
    canvas, el('details', {class: 'mayhem-evidence'}, el('summary', {}, `Exact scatter evidence · ${rows.length} hotspots`),
      new ReportTable({rows, empty: 'No saved hotspot coordinates.', columns: [
        {label: 'Function · source', value: row => row.label}, {label: 'NLOC →', value: row => row.nloc, numeric: true},
        {label: 'CCN ↑', value: row => row.ccn, numeric: true},
      ]}).element)).element];
}

const MAYHEM_RENDER = {overview: mayhemOverview, code: mayhemCode, usage: mayhemUsage, tools: mayhemTools,
  commands: mayhemCommands, backstage: () => [], architecture: mayhemArchitecture, scatter: mayhemScatter};

async function renderAnalytics(scan = false) { return mayhemView.render(scan); }
function mayhemMount() { mayhemView.mount(); }
function analyticsRenderSurface() { mayhemView.renderSurface(); }
function mayhemHealthIndicator() { mayhemView.healthIndicator(); }
