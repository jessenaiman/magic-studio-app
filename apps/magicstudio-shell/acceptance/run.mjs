// Infrastructure gate only: never convert an empty approved suite into a green run.
import { accessSync, appendFileSync, constants, cpSync, mkdirSync, readdirSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import { delimiter, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '../../..');
// One known, git-ignored place for runs. Keep the newest passing run and, if a
// later run failed, that newest failure too; everything else is deleted.
const runsRoot = join(here, 'artifacts');
mkdirSync(runsRoot, { recursive: true });
const prune = () => {
  const runs = readdirSync(runsRoot).filter((name) => /^run-/.test(name)).sort().reverse();
  const passed = (name) => { try { return JSON.parse(readFileSync(join(runsRoot, name, 'result.json'), 'utf8')).status === 'pass'; } catch { return false; } };
  const lastPass = runs.find(passed);
  const lastFail = runs.find((name) => !passed(name));
  const keep = new Set([lastPass, lastFail && (!lastPass || lastFail > lastPass) ? lastFail : undefined]);
  for (const name of runs) if (!keep.has(name)) rmSync(join(runsRoot, name), { recursive: true, force: true });
};
const artifacts = join(runsRoot, `run-${new Date().toISOString().replace(/[:.]/g, '-')}`);
mkdirSync(artifacts);
const finish = (code, status) => {
  rmSync(join(artifacts, 'project'), { recursive: true, force: true }); // never keep the song copy
  writeFileSync(join(artifacts, 'result.json'), JSON.stringify({ status, exitCode: code, finishedAt: new Date().toISOString() }, null, 2));
  prune();
  process.exit(code);
};
// Official test song. Copied (never modified) with only what opening it needs.
const song = process.env.MAGIC_ACCEPTANCE_SONG || '/run/media/jesse/Speedy/media/merriweather/nelly';
const songEntries = ['Nelly.magic', 'project.json', 'nelly.transcript.json', 'assets', 'sequence', 'tracks',
  'storyboard/movie-board.json', 'storyboard/design-draft.json', 'storyboard/design-history',
  'storyboard/storyboard.pdf', 'storyboard/design-1793145236295'];
process.env.MAGIC_ACCEPTANCE_ARTIFACTS = artifacts;
if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `artifacts=${artifacts}\n`);
const approvals = JSON.parse(readFileSync(join(here, 'approved-workflows.json'), 'utf8'));
const executable = (name) => {
  for (const folder of (process.env.PATH || '').split(delimiter)) {
    const path = join(folder, name);
    try { accessSync(path, constants.X_OK); return realpathSync(path); } catch {}
  }
  throw new Error(`Missing installed ${name} on PATH`);
};
const blocked = [];
if (process.platform !== 'linux') blocked.push('This configuration supports Linux official driver only');
if (!process.env.DISPLAY && !process.env.WAYLAND_DISPLAY) blocked.push('No graphical display; use a headed desktop or xvfb-run');
for (const name of ['tauri-driver', 'WebKitWebDriver']) {
  try { executable(name); } catch (error) { blocked.push(error.message); }
}
try {
  const binary = realpathSync(join(repo, 'target/debug/magicstudio-shell'));
  if (binary !== join(repo, 'target/debug/magicstudio-shell')) throw new Error('Native binary must be built in this checkout, not a symlink to an installed app');
  accessSync(binary, constants.X_OK);
  process.env.MAGICSTUDIO_SHELL_BIN = binary;
} catch { blocked.push('Missing repository target/debug/magicstudio-shell: build this checkout with cargo build -p magicstudio-shell'); }
try {
  const cli = executable('magic-cli');
  const shebang = readFileSync(cli, 'utf8').split('\n')[0];
  if (!shebang.startsWith('#!/') || shebang.includes(' ')) throw new Error('magic-cli must be an installed Python console script with an absolute interpreter');
  const provenance = spawnSync(shebang.slice(2), ['-c', 'import importlib.util; print(importlib.util.find_spec("cli_anything.magic.magic_cli").origin)'], { encoding: 'utf8' });
  if (provenance.status !== 0 || realpathSync(provenance.stdout.trim()) !== join(repo, 'agent-harness/cli_anything/magic/magic_cli.py')) throw new Error('Installed magic-cli does not resolve to this checkout; install agent-harness editable in its environment');
  process.env.MAGIC_ACCEPTANCE_CLI = cli;
} catch (error) { blocked.push(error.message); }
const pending = approvals.length === 0;
const record = { timestamp: new Date().toISOString(), status: pending ? 'pending' : blocked.length ? 'blocked' : 'ready', reason: pending ? 'No workflows have Jesse\'s observed-UI approval; zero behavioral coverage' : undefined, blocked, artifacts };
writeFileSync(join(artifacts, 'preflight.json'), JSON.stringify(record, null, 2));
console.log(JSON.stringify(record, null, 2));
if (pending || blocked.length) finish(2, record.status);
// Each entry is added only in a reviewed change after the named human approval.
for (const approval of approvals) {
  if (!approval.reference || !approval.approvedBy || !approval.approvedAt || !/^approved\/[\w-]+\.spec\.mjs$/.test(approval.spec)) throw new Error('Incomplete approval record');
  accessSync(join(here, approval.spec));
}
for (const entry of songEntries) cpSync(join(song, entry), join(artifacts, 'project', entry), { recursive: true, errorOnExist: true, force: false });
process.env.MAGIC_ACCEPTANCE_PROJECT = join(artifacts, 'project', 'Nelly.magic');
const result = spawnSync(process.execPath, [join(here, 'node_modules/@wdio/cli/bin/wdio.js'), 'run', join(here, 'wdio.conf.mjs')], { stdio: 'inherit', env: process.env });
if (result.status !== 0 && process.env.MAGIC_ACCEPTANCE_NO_ISSUES !== '1') fileIssue();
finish(result.status ?? 1, result.status === 0 ? 'pass' : 'fail');

// One GitHub issue per failing step, so an agent can pick it up; reuse an open one.
function fileIssue() {
  let steps = [];
  try { steps = readFileSync(join(artifacts, 'steps.jsonl'), 'utf8').trim().split('\n').map((line) => JSON.parse(line)); } catch {}
  const failed = steps.filter((step) => step.status === 'fail' || step.status === 'blocked');
  const failure = failed.at(-1) ?? { name: 'runner did not reach a workflow step', error: 'See the WDIO reports in the run folder.' };
  const title = `Acceptance failure: ${failure.name}`;
  const gh = (args) => spawnSync('gh', args, { encoding: 'utf8', cwd: repo });
  const open = gh(['issue', 'list', '--state', 'open', '--search', `"${title}" in:title`, '--json', 'number,title']);
  const existing = open.status === 0 ? JSON.parse(open.stdout || '[]').find((issue) => issue.title === title) : undefined;
  const body = [
    `Native acceptance run \`${artifacts.split('/').pop()}\` failed (build of \`${repo}\`).`,
    '', `**Step:** ${failure.name}`,
    failure.expectations ? `**Expected:** ${Object.values(failure.expectations).join('; ')}` : '',
    '', '```', String(failure.error ?? '').split('\n').slice(0, 15).join('\n'), '```',
    '', `Evidence (local, auto-pruned): \`${artifacts}\`. Rerun with \`npm test\` in \`apps/magicstudio-shell/acceptance\`.`,
  ].join('\n');
  const posted = existing ? gh(['issue', 'comment', String(existing.number), '--body', body]) : gh(['issue', 'create', '--title', title, '--body', body]);
  console.log(posted.status === 0 ? `Failure reported: ${posted.stdout.trim()}` : `Could not report failure to GitHub: ${posted.stderr.trim()}`);
}
