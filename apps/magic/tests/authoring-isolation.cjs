const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict');
const c=vm.createContext({});vm.runInContext(fs.readFileSync(path.join(__dirname,'../../magic-cli/ui/lyrics-layout.js'),'utf8'),c);
assert.equal(c.authoringVisibility('design',false).words,true);assert.equal(c.authoringVisibility('design',false).drawings,false);
assert.equal(c.authoringVisibility('motion',false).words,false);assert.equal(c.authoringVisibility('motion',false).drawings,true);
for(const mode of ['design','motion']){assert.equal(c.authoringVisibility(mode,true).words,true);assert.equal(c.authoringVisibility(mode,true).drawings,true);}
console.log('Authoring layers are isolated by default; combined review is explicit');
