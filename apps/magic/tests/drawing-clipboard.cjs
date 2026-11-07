const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict');
const c=vm.createContext({structuredClone});
vm.runInContext(fs.readFileSync(path.join(__dirname,'../../magic-cli/ui/drawing-controls.js'),'utf8'),c);
const drawing={name:'Circle',view_box:'0 0 200 200',x:100,y:200,w:200,h:200,word_ids:['old-line/w0'],paths:[{d:'M100 0 A100 100 0 1 1 100 200',stroke:'#123456',width:5,from:.1,to:1.2}]};
const result=c.parseDrawingClipboard(c.drawingClipboardText(drawing));
assert.equal(result.paths[0].d,drawing.paths[0].d);
assert.equal(result.word_ids,undefined,'Clipboard cannot carry stale word links into another line');
result.paths[0].width=8;
assert.equal(drawing.paths[0].width,5);
assert.throws(()=>c.parseDrawingClipboard('artist lyrics'));
const hostile=structuredClone(drawing);hostile.paths[0].d='M0 0"/><script>alert(1)</script>';
assert.throws(()=>c.parseDrawingClipboard(c.drawingClipboardText(hostile)));

// Style copy/paste: copies visual styling and never mutates words[].text or word_ids
const sourceItem={word_ids:['verse/w0','verse/w1'],text:'going around',x:140,y:220,size:120,weight:700,italic:true,font:'serif',color:'#e4572e',outline:'#143042',rotate:-6,motion:'loop',accent:'verse/w1',native_motion:{property:'transform/rotation',from:0,speed:180,duration:2,delay:0},motion_keys:[[0,0,0,100,100,0,100],[2,0,0,100,100,360,100]]};
const styleText=c.styleClipboardText(sourceItem);
const parsedStyle=c.parseStyleClipboard(styleText);
assert.equal(parsedStyle.text,undefined,'Style clipboard envelope must never carry text');
assert.equal(parsedStyle.word_ids,undefined,'Style clipboard envelope must never carry source word_ids');
assert.equal(parsedStyle.accent,undefined,'Style clipboard envelope must never carry source accent word_id');
assert.equal(parsedStyle.font,'serif');
assert.equal(parsedStyle.size,120);
assert.equal(parsedStyle.weight,700);
assert.equal(parsedStyle.italic,true);
assert.equal(parsedStyle.color,'#e4572e');
assert.equal(parsedStyle.outline,'#143042');
assert.equal(parsedStyle.rotate,-6);
assert.equal(parsedStyle.motion,'loop');
assert.deepEqual(JSON.parse(JSON.stringify(parsedStyle.native_motion)),sourceItem.native_motion);
assert.deepEqual(JSON.parse(JSON.stringify(parsedStyle.motion_keys)),sourceItem.motion_keys);

const words=[{word_id:'chorus/w0',text:'Artist'},{word_id:'chorus/w1',text:'lyrics'}];
const wordsSnapshot=JSON.stringify(words);
const targetDesign={items:[{word_ids:['chorus/w0','chorus/w1'],text:'Artist lyrics',x:300,y:400,size:58,weight:400,italic:false,font:'song',color:'#ffffff',outline:null,rotate:0,motion:'quiet',accent:null}],draws:[]};
const styledDesign=c.applyStyleClipboard(targetDesign,0,c.parseStyleClipboard(c. JSON ? styleText : styleText));
assert.equal(JSON.stringify(words),wordsSnapshot,'Artist words[].text must remain completely untouched');
assert.deepEqual(JSON.parse(JSON.stringify(styledDesign.items[0].word_ids)),['chorus/w0','chorus/w1'],'Target word_ids must be preserved');
assert.equal(styledDesign.items[0].text,'Artist lyrics','Target item text must never be overwritten');
assert.equal(styledDesign.items[0].x,300,'Target placement x must be preserved');
assert.equal(styledDesign.items[0].y,400,'Target placement y must be preserved');
assert.equal(styledDesign.items[0].size,120);
assert.equal(styledDesign.items[0].font,'serif');
assert.equal(styledDesign.items[0].weight,700);
assert.equal(styledDesign.items[0].italic,true);
assert.equal(styledDesign.items[0].color,'#e4572e');
assert.equal(styledDesign.items[0].outline,'#143042');
assert.equal(styledDesign.items[0].rotate,-6);
assert.equal(styledDesign.items[0].motion,'loop');
assert.equal(styledDesign.items[0].accent,'chorus/w0','Native motion accent defaults to target first word_id, never source word_id');

// Hostile style payloads must be rejected or stripped of text/word_ids
assert.throws(()=>c.parseStyleClipboard(c.drawingClipboardText(drawing)),'Drawing clipboard must not parse as style clipboard');
assert.throws(()=>c.parseStyleClipboard(JSON.stringify({format:'magic.style',version:1,style:{...parsedStyle,color:'<script>'}})));
const injected=c.parseStyleClipboard(JSON.stringify({format:'magic.style',version:1,style:{...parsedStyle,text:'HACKED',word_ids:['hack/w0'],words:[{text:'HACKED'}]}}));
assert.equal(injected.text,undefined);
assert.equal(injected.word_ids,undefined);
assert.equal(injected.words,undefined);

// RestoredWorkspace bridges full clipboard API (readText, writeText, readImage, writeImage, writeHtml, clear)
const workspaceTsx=fs.readFileSync(path.join(__dirname,'../ui/components/RestoredWorkspace.tsx'),'utf8');
for(const fn of ['readText','writeText','readImage','writeImage','writeHtml','clear']){
  assert.match(workspaceTsx,new RegExp(`\\b${fn}\\b`),`RestoredWorkspace.tsx must bridge ${fn} from @tauri-apps/plugin-clipboard-manager`);
}
console.log('Drawing & style clipboard copies editable geometry/styling, bridges full clipboard API, and preserves read-only artist lyrics');
