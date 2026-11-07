const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict');
const messages=[];
let refreshes=0;
const context=vm.createContext({window:{},addEventListener(){},wordsCurrent:()=>({animation_id:'verse'}),wordsLabel:()=> 'Verse',wordsCommand:async()=>{throw new Error('Native save failed')},wordsOpen:async()=>{refreshes++},toast:message=>messages.push(String(message))});
vm.runInContext(fs.readFileSync(path.join(__dirname,'../../magic-cli/ui/words-flow.js'),'utf8'),context);
context.window.wordsStageController.flushPendingEdits=async()=>true;
(async()=>{await context.window.wordsStageController.save();assert.equal(refreshes,0,'A failed native save must not refresh away the current editing state');assert.ok(messages.some(x=>x.includes('Native save failed')));assert.ok(!messages.some(x=>x.startsWith('Saved ')),'Never report a native save failure as success');console.log('Native save failures remain visible');})().catch(e=>{console.error(e);process.exitCode=1});
