import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
const here = dirname(fileURLToPath(import.meta.url));
const approvals = JSON.parse(readFileSync(join(here, 'approved-workflows.json'), 'utf8'));
export const config = {
  runner: 'local',
  logLevel: 'warn', // standard reports only; rerun for more detail
  specs: approvals.map(({ spec }) => join(here, spec)),
  maxInstances: 1,
  capabilities: [{ browserName: 'wry', 'tauri:options': { application: process.env.MAGICSTUDIO_SHELL_BIN, args: [process.env.MAGIC_ACCEPTANCE_PROJECT] } }],
  services: [['@wdio/tauri-service', {
    appBinaryPath: process.env.MAGICSTUDIO_SHELL_BIN,
    // tauri-driver forwards tauri:options.args: the app opens the disposable song directly.
    appArgs: [process.env.MAGIC_ACCEPTANCE_PROJECT],
    // Pinned service 1.5.1 source normalizes deprecated 'official' to 'external'.
    driverProvider: 'external',
    autoInstallTauriDriver: false,
    autoDownloadEdgeDriver: false,
  }]],
  framework: 'mocha',
  reporters: ['spec', ['json', { outputDir: process.env.MAGIC_ACCEPTANCE_ARTIFACTS }]],
  outputDir: process.env.MAGIC_ACCEPTANCE_ARTIFACTS,
  mochaOpts: { timeout: 120000 },
  connectionRetryCount: 0,
};
