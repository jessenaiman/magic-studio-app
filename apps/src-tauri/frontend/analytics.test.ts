import {test, expect} from 'bun:test';
import {readFileSync} from 'node:fs';
import {runInNewContext} from 'node:vm';

const source = new Bun.Transpiler({loader: 'ts'}).transformSync(readFileSync(new URL('../ui/analytics.ts', import.meta.url), 'utf8'));
const model = runInNewContext(`${source}; ({mayhemChartSeries, mayhemScatterRows})`);

test('unavailable sources never become zero-valued chart measurements', () => {
  expect(model.mayhemChartSeries({complexity: {available: false, languages: [{lang: 'rust', lines: 20}]}, usage: {available: false, agents: [{name: 'A', today_tokens: 2}]}})).toEqual({languages: [], usage: [], history: [], checks: []});
});

test('chart series preserve measured zero and omit unknown and invalid values', () => {
  const series = model.mayhemChartSeries({complexity: {available: true, languages: [{lang: 'rust', lines: 0}, {lang: 'css', lines: null}, {lang: 'html', lines: -1}], history: [{measured_at_unix_secs: 10, max_ccn: 5}, {measured_at_unix_secs: 20}]}, usage: {available: true, agents: [{name: 'A', today_tokens: 4}, {name: 'B'}]}, audit: {available: true, tools: [{label: 'Lint', findings: 0}, {label: 'Unknown'}]}});
  expect(series.languages).toEqual([{label: 'Rust', value: 0}]);
  expect(series.usage).toEqual([{label: 'A', value: 4}]);
  expect(series.history).toEqual([{label: new Date(10000).toLocaleString(), value: 5}]);
  expect(series.checks).toEqual([{label: 'Lint', value: 0}]);
});

test('scatter only contains saved measured hotspots with both coordinates', () => {
  expect(model.mayhemScatterRows({available: false, hotspots: [{ccn: 4, nloc: 10}]})).toEqual([]);
  expect(model.mayhemScatterRows({available: true, hotspots: [{name: 'f', file: 'x.rs', ccn: 4, nloc: 10}, {ccn: null, nloc: 2}, {ccn: 8}]})).toEqual([{label: 'f · x.rs:?', ccn: 4, nloc: 10}]);
});


test('failed Jev invocation preserves the previous report and exposes the real error', async () => {
  const controller = runInNewContext(`${source}; new AnalyticsViewController()`, {
    $: () => ({disabled: false}),
    invoke: async () => { throw new Error('Jev unavailable'); },
  });
  controller.renderSurface = () => {};
  controller.data = {jev: {state: 'saved'}};
  await controller.runJev();
  expect(controller.data.jev.state).toBe('saved');
  expect(controller.judgeError).toBe('Error: Jev unavailable');
  expect(controller.judging).toBe(false);
});

