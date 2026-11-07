const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict');
const context=vm.createContext({window:{},localStorage:{getItem:()=>null},document:{createElement:()=>({getContext:()=>({})})},ResizeObserver:class{observe(){}},addEventListener(){},wordsClamp:v=>Math.max(0,Math.min(1,v))});
vm.runInContext(fs.readFileSync(path.join(__dirname,'../../magic-cli/ui/words-design.js'),'utf8'),context);
const item={x:100,y:200,size:80,rotate:0,native_motion:{property:'transform/rotation'},motion_keys:[[0,0,0,100,100,0,100],[2,0,0,100,100,360,100]]};
const vars={};
context.window.wordsTimelineController.applyWordKeys({style:{setProperty:(key,value)=>vars[key]=value}},item,1);
assert.equal(vars['--y'],'200px','Adding motion preserves the authored top position');
assert.equal(vars['--rot'],'180deg');
console.log('Native motion preserves placement while interpolating rotation');
