const fs=require('node:fs'),vm=require('node:vm'),path=require('node:path'),assert=require('node:assert/strict'),ts=require('../node_modules/typescript');
const context=vm.createContext({exports:{}});
vm.runInContext(ts.transpileModule(fs.readFileSync(path.join(__dirname,'../ui/artwork-transfer.ts'),'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText,context);
(async()=>{
 const calls=[];const run=async(id,params)=>{calls.push({id,params});};
 await assert.rejects(context.exports.importDroppedArtwork(['/tmp/a.png','/tmp/source.magic'],'/song.magic',run));
 assert.equal(calls.length,0,'Validate the complete drop before importing');
 assert.equal(await context.exports.importDroppedArtwork(['/tmp/a.png','/tmp/a.png','/tmp/b.webp'],'/song.magic',run),2);
 assert.ok(calls.every(c=>c.id==='project.add_asset'&&c.params._expected_project==='/song.magic'&&c.params.kind==='image'));
 assert.equal(calls[0].params.image,'a');
 await assert.rejects(context.exports.importDroppedArtwork(['/tmp/a.png'],'',run));
 console.log('Native artwork drops reuse guarded project imports and reject mixed unsupported batches');
})().catch(error=>{console.error(error);process.exitCode=1;});
