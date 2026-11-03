import {init, use} from 'echarts/core';
import {BarChart, LineChart} from 'echarts/charts';
import {GridComponent, TooltipComponent, AriaComponent} from 'echarts/components';
import {CanvasRenderer} from 'echarts/renderers';

use([BarChart, LineChart, GridComponent, TooltipComponent, AriaComponent, CanvasRenderer]);

type Row = {label: string; value: number};
function mount(node: HTMLElement, rows: Row[], kind: 'bar' | 'line', color: string, unit: string) {
  const style = getComputedStyle(node);
  const muted = style.getPropertyValue('--muted').trim();
  const line = style.getPropertyValue('--line').trim();
  const chart = init(node, null, {renderer: 'canvas'});
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

// ECharts owns branch layout, connecting lines, expansion and pan/zoom.
import {TreeChart} from 'echarts/charts';
use([TreeChart]);
function tree(node: HTMLElement, commands: {id:string;label:string;params:string}[]) {
  const groups=new Map<string,any>();
  for (const cmd of commands) {
    const group=cmd.id.split('.')[0];
    if (!groups.has(group)) groups.set(group,{name:group,children:[],collapsed:true});
    groups.get(group).children.push({name:cmd.id,description:cmd.label,params:cmd.params});
  }
  const chart=init(node,null,{renderer:'canvas'});
  chart.setOption({animation:false,tooltip:{trigger:'item',renderMode:'richText',
    formatter:(p:any)=>[p.name,p.data.description,p.data.params].filter(Boolean).join('\n')},
    series:[{type:'tree',data:[{name:'magic-cli',children:[...groups.values()]}],
      top:12,bottom:12,left:'10%',right:'32%',roam:true,expandAndCollapse:true,initialTreeDepth:1,
      label:{color:'#dce5ef',position:'left'},leaves:{label:{position:'right'}}}]});
  const observer=new ResizeObserver(()=>chart.resize()); observer.observe(node);
  return {dispose(){observer.disconnect();chart.dispose();}};
}
Object.assign(globalThis,{magicMayhemTree:tree});
