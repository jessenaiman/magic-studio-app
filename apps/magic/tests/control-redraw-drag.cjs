const fs = require('node:fs'), vm = require('node:vm'), assert = require('node:assert/strict');
const ts = require('../node_modules/typescript');
let moves = 0, ups = 0, current;
const doc = {defaultView:{MouseEvent:class{constructor(type,init){Object.assign(this,init);this.type=type;}},PointerEvent:class{constructor(type,init){Object.assign(this,init);this.type=type;}}},elementFromPoint:()=>current,activeElement:null};
function element() {return {isConnected:true,tagName:'DIV',ownerDocument:doc,closest:()=>null,dispatchEvent(event){
  if (!this.isConnected) return true;
  if (event.type==='pointerdown' || event.type==='pointermove') {
    if(event.type==='pointermove') moves++;
    this.isConnected=false; current=element();
  }
  if(event.type==='pointerup') ups++;
  return true;
}};}
current=element();
const context=vm.createContext({exports:{},document:doc,HTMLIFrameElement:class{}});
vm.runInContext(ts.transpileModule(fs.readFileSync(require('node:path').join(__dirname,'../ui/control-input.ts'),'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText,context);
context.exports.drag({from:{x:10,y:20},to:{x:70,y:80},steps:4});
assert.equal(moves,4,'All move events must reach the live document after a redraw');
assert.equal(ups,1,'The drag must end on a connected element');
console.log('Control drag survives redraws on pointer down and move');
ups=0; current=element();
context.exports.click({x:10,y:20});
assert.equal(ups,1,'Selecting a word that redraws must release its drag state');
const events=[];
doc.defaultView.Event=class{constructor(type){this.type=type;}};
context.exports.commitValueEvents({tagName:'INPUT',isConnected:true,ownerDocument:doc,dispatchEvent:event=>events.push(event.type)});
assert.deepEqual(events,['input','change'],'A committed value must reach change-based drawing controls');
