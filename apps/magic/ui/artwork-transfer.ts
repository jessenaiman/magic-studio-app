// The file picker and native file drops use the same engine import command.
export async function importDroppedArtwork(paths: string[], project: string, run: (id: string, params: Record<string, unknown>) => Promise<unknown>): Promise<number> {
  const files = [...new Set(paths)];
  if (!project) throw new Error("Open a project before dropping artwork");
  if (!files.length) return 0;
  const unsupported = files.filter(path => !/\.(png|jpe?g|webp)$/i.test(path));
  if (unsupported.length) throw new Error("Artwork drops accept PNG, JPG and WebP files. Use the project controls for documents and audio.");
  let imported = 0;
  for (const path of files) {
    const name = path.split(/[/\\]/).pop()!.replace(/\.[^.]+$/, "");
    try {
      await run("project.add_asset", {path, kind:"image", name, image:name, load:false, _expected_project:project});
      imported++;
    } catch (error) { throw new Error(`Imported ${imported} of ${files.length} artwork files: ${String(error)}`); }
  }
  return imported;
}
