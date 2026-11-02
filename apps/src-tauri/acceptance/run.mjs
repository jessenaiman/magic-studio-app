// Infrastructure gate only: never convert an empty approved suite into a green run.
import { accessSync, appendFileSync, constants, mkdtempSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { delimiter, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, '../../..');
const artifacts = mkdtempSync(join(tmpdir(), 'magic-acceptance-'));
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
  const binary = realpathSync(join(repo, 'target/debug/magic-studio'));
  if (binary !== join(repo, 'target/debug/magic-studio')) throw new Error('Native binary must be built in this checkout, not a symlink to an installed app');
  accessSync(binary, constants.X_OK);
  process.env.MAGIC_STUDIO_BIN = binary;
} catch { blocked.push('Missing repository target/debug/magic-studio: build this checkout with cargo build -p magic-studio'); }
try {
  const cli = realpathSync(join(repo, 'target/debug/magic-cli'));
  if (cli !== join(repo, 'target/debug/magic-cli')) throw new Error('magic-cli must be built in this checkout, not a symlink to an installed copy');
  accessSync(cli, constants.X_OK);
  process.env.MAGIC_ACCEPTANCE_CLI = cli;
} catch (error) { blocked.push(error.message.startsWith('magic-cli must') ? error.message : 'Missing repository target/debug/magic-cli: build this checkout with cargo build -p magic-cli'); }
const pending = approvals.length === 0;
const record = { timestamp: new Date().toISOString(), status: pending ? 'pending' : blocked.length ? 'blocked' : 'ready', reason: pending ? 'No workflows have Jesse\'s observed-UI approval; zero behavioral coverage' : undefined, blocked, artifacts };
writeFileSync(join(artifacts, 'preflight.json'), JSON.stringify(record, null, 2));
console.log(JSON.stringify(record, null, 2));
if (pending || blocked.length) process.exit(2);
// Each entry is added only in a reviewed change after the named human approval.
for (const approval of approvals) {
  if (!approval.reference || !approval.approvedBy || !approval.approvedAt || !/^approved\/[\w-]+\.spec\.mjs$/.test(approval.spec)) throw new Error('Incomplete approval record');
  accessSync(join(here, approval.spec));
}
const result = spawnSync(process.execPath, [join(here, 'node_modules/@wdio/cli/bin/wdio.js'), 'run', join(here, 'wdio.conf.mjs')], { stdio: 'inherit', env: process.env });
process.exit(result.status ?? 1);
