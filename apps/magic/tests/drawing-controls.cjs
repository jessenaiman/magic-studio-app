const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict');
const c=vm.createContext({structuredClone});vm.runInContext(fs.readFileSync(path.join(__dirname,'../../magic-cli/ui/drawing-controls.js'),'utf8'),c);
const original={items:[{word_ids:['round']}],draws:[{name:'Circle',x:20,y:30,w:200,h:200,paths:[{d:'M0 0L1 1',from:1,to:2,width:4,stroke:'#123456'},{d:'M1 1L2 2',from:2,to:3,width:4,stroke:'#123456'}]}]};
const before=JSON.stringify(original);
const edited=c.patchDrawing(original,0,{x:50,start:2,end:6,strokeWidth:7,color:'#ffffff',word_ids:['round']});
assert.equal(edited.draws[0].paths[0].from,2);assert.equal(edited.draws[0].paths[0].to,4);assert.equal(edited.draws[0].paths[1].to,6);
assert.equal(edited.draws[0].x,50);assert.equal(edited.draws[0].paths[1].width,7);assert.deepEqual(edited.items,original.items);
assert.equal(JSON.stringify(original),before);assert.throws(()=>c.patchDrawing(original,0,{start:3,end:1}));
console.log('Drawing edits preserve source items and multi-stroke timing proportions');
