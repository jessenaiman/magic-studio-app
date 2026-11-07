const vm = require('node:vm'), fs = require('node:fs'), assert = require('node:assert/strict');
const context = vm.createContext({});
vm.runInContext(fs.readFileSync(require('node:path').join(__dirname, '../ui/workspaces/animation-connection.js'),'utf8'), context);
const check = state => vm.runInContext(`animationClipExportProblem(${JSON.stringify(state)}, 'Robot / Dance')`, context);
assert.equal(check({}), null);
assert.match(check({sheetsById:{'Robot / Dance':{sequence:{tracksByObject:{Bone:{trackData:{r:{keyframes:[{position:0,value:0},{position:1,value:30}]}}}}}}}}), /timeline/i);
assert.match(check({sheetsById:{'Robot / Dance':{staticOverrides:{byObject:{Bone:{rotation:20}}}}}}), /timeline/i);
assert.equal(check({sheetsById:{'Other / Walk':{staticOverrides:{byObject:{Bone:{rotation:20}}}}}}), null);
console.log('Export guard cases passed');
