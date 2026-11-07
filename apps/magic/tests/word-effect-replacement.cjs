const fs = require('node:fs'), path = require('node:path'), vm = require('node:vm'), assert = require('node:assert/strict');
let design;
const context = vm.createContext({
  window: {}, addEventListener() {},
  el: (tag, props, ...children) => ({tag, props, children}),
  wordsMoves: () => [['quiet', 'Quiet', 'Still', '']],
  wordsEffectCraftTools: () => ({text_presets: [{id:'fade',name:'Fade'}]}),
  wordsDesignEdit: (_anim, _line, edit) => edit(design),
});
vm.runInContext(fs.readFileSync(path.join(__dirname, '../../magic-cli/ui/words-flow.js'), 'utf8'), context);
const buttons = node => typeof node === 'object' ? [ ...(node.tag === 'button' ? [node] : []), ...node.children.flatMap(buttons)] : [];
for (const choice of [0, 1]) {
  design = {items:[{word_ids:['round'],accent:'round',motion:'loop',native_motion:{property:'transform/rotation',speed:180,duration:2},motion_keys:[[0,0,0,100,100,0,100],[2,0,0,100,100,360,100]]}]};
  const panel = context.window.wordsStageController.colMotionAndMarks({}, 'line', design, 0);
  buttons(panel)[choice].props.onclick();
  assert.equal(design.items[0].native_motion, undefined, 'Replacing motion must remove the native configuration that save would regenerate');
  assert.equal(design.items[0].motion_keys, undefined);
  assert.deepEqual(design.items[0].word_ids, ['round']);
}
console.log('Effect replacement clears the previous native curve and preserves artist word IDs');
