// The file picker and native file drops use the same engine import command.
export async function importDroppedArtwork(paths: string[], project: string, run: (id: string, params: Record<string, unknown>) => Promise<unknown>): Promise<number> {
  const files = [...new Set(paths)];
  if (!project) throw new Error("Open a project before dropping artwork");
  if (!files.length) return 0;
  let imported = 0;
  for (const path of files) {
    // `image` names the assets/images/<name>/ folder an outside file is copied into.
    const name = path.split(/[/\\]/).pop()!.replace(/\.[^.]+$/, "");
    try {
      // The engine refuses a file that is not an image and names it.
      await run("project.add_asset", {path, kind:"image", name, image:name, load:false, _expected_project:project});
      imported++;
    } catch (error) { throw new Error(`Imported ${imported} of ${files.length} artwork files: ${String(error)}`); }
  }
  return imported;
}
