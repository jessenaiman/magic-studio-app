// Theatre limits project IDs to 32 characters. Canonical paths still distinguish projects.
export async function theatreProjectId(path){
 const digest=await crypto.subtle.digest('SHA-256',new TextEncoder().encode(path));
 return 'Magic Studio '+Array.from(new Uint8Array(digest).slice(0,8),n=>n.toString(16).padStart(2,'0')).join('');
}

// Imported GLB clip names can exceed Theatre's 64-character path component limit.
// Keep short saved sheet names unchanged; suffix long names with a stable digest.
export async function theatreSheetId(name) {
 return (await Promise.all(name.split('/').map(async part => {
  const text=part.trim();
  if(text.length<=64)return text;
  const digest=await crypto.subtle.digest('SHA-256',new TextEncoder().encode(text));
  const suffix=Array.from(new Uint8Array(digest).slice(0,6),n=>n.toString(16).padStart(2,'0')).join('');
  return text.slice(0,48)+'-'+suffix;
 }))).join(' / ');
}
