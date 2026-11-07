const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const caps=JSON.parse(fs.readFileSync(path.join(__dirname,'../capabilities/default.json'),'utf8'));
const perms=caps.permissions||[];

// Find http entry (bare string or object)
const httpEntry = perms.find(p => p==='http:default' || (p&&typeof p==='object'&&p.identifier==='http:default'));
assert(httpEntry, 'http:default entry must exist');
assert.notEqual(httpEntry, 'http:default', 'http:default must be a scoped object, not a bare string (Tauri v2 requires explicit {identifier,allow,deny} scope)');

const allow = httpEntry.allow||[];
const deny = httpEntry.deny||[];
assert.ok(Array.isArray(allow), 'allow must be an array');
assert.ok(Array.isArray(deny), 'deny must be an array');

// Allowlist: only loopback for HyperFrames preview (mirrors CSP connect-src http://127.0.0.1:* / localhost:*)
const allowedUrls = allow.map(e => e.url||e);
assert.ok(allowedUrls.length>0, 'allow must list at least loopback destinations');
assert.ok(allowedUrls.every(u => u.startsWith('http://127.0.0.1:') || u.startsWith('http://localhost:')), `allow must be loopback only, got ${JSON.stringify(allowedUrls)}`);
assert.ok(!allowedUrls.some(u => u.includes('*') && !u.includes('127.0.0.1') && !u.includes('localhost')), 'allow must not contain wildcard https://*');
assert.ok(!allowedUrls.some(u => u.includes('api.openai.com') || u.includes('api.storyteller.ai')), 'allow must not include credentialed providers');

// Denylist: credentialed / authenticated providers must be explicitly denied for JS (Rust-only)
const deniedUrls = deny.map(e => e.url||e);
assert.ok(deniedUrls.some(u => u.includes('api.openai.com')), 'deny must include api.openai.com (Rust ureq, key in Rust only)');
assert.ok(deniedUrls.some(u => u.includes('api.storyteller.ai')), 'deny must include api.storyteller.ai (Rust/backend only)');

// Rust must keep credentials: image_generation.rs must use Authorization header via ureq (not JS)
const imgGen = fs.readFileSync(path.join(__dirname,'../../../crates/engine/src/image_generation.rs'),'utf8');
assert.ok(imgGen.includes('Authorization') && imgGen.includes('Bearer'), 'image_generation.rs must keep Bearer auth in Rust (ureq)');
assert.ok(!imgGen.includes('plugin-http') && !imgGen.includes('@tauri-apps/plugin-http'), 'image_generation.rs must not call JS http plugin for credentials');

const skills = fs.readFileSync(path.join(__dirname,'../../../crates/engine/src/skills_registry.rs'),'utf8');
assert.ok(skills.includes('curl') || skills.includes('--resolve'), 'skills_registry.rs must keep SSRF-guarded curl in Rust (no JS http for public IPv4 fetch)');

console.log('HTTP scope is explicit, loopback-only for JS; credentials stay in Rust for OpenAI/skills');
