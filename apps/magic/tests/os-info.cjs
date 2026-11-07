const fs=require('node:fs'),path=require('node:path'),vm=require('node:vm'),assert=require('node:assert/strict'),ts=require('../node_modules/typescript');
const context=vm.createContext({exports:{}});
vm.runInContext(ts.transpileModule(fs.readFileSync(path.join(__dirname,'../ui/os-info.ts'),'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText,context);
const { formatOsInfo, isMacOSPlatform, isWindowsPlatform, modifierLabel, isUnixFamily } = context.exports;

// 1. formatOsInfo aggregates all platform fields without touching lyrics
const info = formatOsInfo({ platform:'darwin', family:'unix', arch:'aarch64', version:'14.0', osType:'macos', exeExtension:'', eol:'\n', locale:'en-US' });
assert.equal(info.platform,'macos','platform must be normalized to os:*}');
assert.equal(info.family,'unix');
assert.equal(info.arch,'aarch64');
assert.equal(info.version,'14.0');
assert.equal(info.osType,'macos');
assert.equal(info.locale,'en-US');
assert.equal(info.eol,'\n');

// darwin input -> macos normalized, windows -> exe, unknown -> allowed list
assert.equal(formatOsInfo({ platform:'darwin', family:'unix', arch:'x86_64', version:'', osType:'macos', exeExtension:'', eol:'\n', locale:null }).platform,'macos');
assert.equal(formatOsInfo({ platform:'windows', family:'windows', arch:'x86_64', version:'11', osType:'windows', exeExtension:'exe', eol:'\r\n', locale:'en-GB' }).exeExtension,'exe');

// format must not mutate artist words: pure object, no text field
assert.equal(info.text, undefined, 'OS info must never carry lyric text');
assert.equal(formatOsInfo({ platform:'linux', family:'unix', arch:'x86_64', version:'', osType:'linux', exeExtension:'', eol:'\n', locale:null }).platform,'linux');

// 2. isMacOSPlatform
assert.equal(isMacOSPlatform('macos'), true);
assert.equal(isMacOSPlatform('linux'), false);
assert.equal(isMacOSPlatform('windows'), false);

// 3. isWindowsPlatform
assert.equal(isWindowsPlatform('windows'), true);
assert.equal(isWindowsPlatform('linux'), false);

// 4. modifierLabel - Cmd on mac, Ctrl elsewhere (for GlobalShortcut future)
assert.equal(modifierLabel('macos'), 'Cmd');
assert.equal(modifierLabel('linux'), 'Ctrl');
assert.equal(modifierLabel('windows'), 'Ctrl');

// 5. isUnixFamily + unknown platform validation
assert.equal(isUnixFamily('unix'), true);
assert.equal(isUnixFamily('windows'), false);
assert.throws(()=>formatOsInfo({ platform:'bogus-os', family:'unix', arch:'x86_64', version:'', osType:'linux', exeExtension:'', eol:'\n', locale:null }), /Unknown platform/);
assert.throws(()=>formatOsInfo({ platform:'linux', family:'weird', arch:'x86_64', version:'', osType:'linux', exeExtension:'', eol:'\n', locale:null }), /Unknown family/);

console.log('OS info helpers aggregate platform diagnostics and preserve read-only lyrics');
