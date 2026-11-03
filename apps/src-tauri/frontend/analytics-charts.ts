import {init, use} from 'echarts/core';
import {BarChart, LineChart} from 'echarts/charts';
import {GridComponent, TooltipComponent, AriaComponent} from 'echarts/components';
import {SVGRenderer} from 'echarts/renderers';

use([BarChart, LineChart, GridComponent, TooltipComponent, AriaComponent, SVGRenderer]);

type Row = {label: string; value: number};
function mount(node: HTMLElement, rows: Row[], kind: 'bar' | 'line', color: string, unit: string) {
  const style = getComputedStyle(node);
  const muted = style.getPropertyValue('--muted').trim();
  const line = style.getPropertyValue('--line').trim();
  const chart = init(node, null, {renderer: 'svg'});
  try {
    chart.setOption({
      animation: false,
      aria: {enabled: true, description: `${node.getAttribute('aria-label')}. ${rows.map(row => `${row.label}: ${row.value} ${unit}`).join('; ')}`},
      textStyle: {fontFamily: style.fontFamily, color: muted},
      grid: {top: 24, left: 14, right: 22, bottom: 12, containLabel: true},
      tooltip: {trigger: 'axis', renderMode: 'richText', confine: true, valueFormatter: (value: number) => `${value.toLocaleString()} ${unit}`},
      xAxis: kind === 'line'
        ? {type: 'category', data: rows.map(row => row.label), axisLabel: {color: muted, hideOverlap: true}, axisLine: {lineStyle: {color: line}}}
        : {type: 'value', min: 0, axisLabel: {color: muted}, splitLine: {lineStyle: {color: line}}},
      yAxis: kind === 'line'
        ? {type: 'value', min: 0, axisLabel: {color: muted}, splitLine: {lineStyle: {color: line}}}
        : {type: 'category', inverse: true, data: rows.map(row => row.label), axisLabel: {color: muted, width: 115, overflow: 'truncate'}, axisTick: {show: false}, axisLine: {show: false}},
      series: [{type: kind, data: rows.map(row => row.value), color, barMaxWidth: 22, showSymbol: true, symbolSize: 7,
        lineStyle: {width: 2}, itemStyle: {borderRadius: kind === 'bar' ? [0, 4, 4, 0] : 0}, label: {show: kind === 'bar', position: 'right', color: muted}}],
    });
  } catch (error) {
    chart.dispose();
    throw error;
  }
  const observer = new ResizeObserver(() => { if (node.isConnected && node.clientWidth) chart.resize(); });
  observer.observe(node);
  return {dispose() { observer.disconnect(); chart.dispose(); }};
}

Object.assign(globalThis, {magicMayhemCharts: {mount}});
