const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict');
const context=vm.createContext({});
vm.runInContext(fs.readFileSync(path.join(__dirname,'../../magic-cli/ui/lyrics-layout.js'),'utf8'),context);
assert.equal(context.authoringSongDuration({project:'nelly',duration:81.6},'nelly',117.5),81.6);
assert.equal(context.authoringSongDuration({project:'other',duration:81.6},'nelly',117.5),117.5);
assert.equal(context.authoringSongDuration({project:'nelly',duration:NaN},'nelly',117.5),117.5);
console.log('Transport duration uses the current recording rather than stale clip ends');
