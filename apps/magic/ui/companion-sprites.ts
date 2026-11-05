// Companion sprite loader.
//
// The manifest is swappable data: any character may be re-pointed at other
// artwork without touching code. A missing, unreadable or partial manifest
// degrades to the shipped ASCII frames instead of showing nothing, so the
// valid-state fallback is a real fallback and not a decoration.
class CompanionSpriteCatalog {
  images = new Map();
  manifest: {version: number, characters: Record<string, any>} = {version: 0, characters: {}};
  manifestUrl: string;
  ready = false;

  constructor(manifestUrl = "companions.manifest.json") {
    this.manifestUrl = manifestUrl;
  }

  load = async () => {
    try {
      const response = await fetch(this.manifestUrl);
      if (response.ok) {
        const parsed = await response.json();
        if (parsed && typeof parsed === "object" && parsed.characters) {
          this.manifest.version = parsed.version || 0;
          this.manifest.characters = parsed.characters;
        }
      }
    } catch (_) {
      // Fall through to the ASCII fallback.
    }
    this.ready = true;
    return this.manifest;
  };

  character = (name: string) => {
    return this.manifest.characters && this.manifest.characters[name] ? this.manifest.characters[name] : null;
  };

  // A pose must exist and be a real file; anything else falls back to rest.
  poseFor = (name: string, pose: string) => {
    const character_ = this.character(name);
    if (!character_ || !character_.poses) return null;
    const wanted = character_.poses[pose];
    const fallback = character_.poses.rest;
    const src = wanted || fallback;
    return src ? {src, pose: wanted ? pose : "rest"} : null;
  };

  image = (name: string, pose: string) => {
    const key = `${name}:${pose}`;
    if (this.images.has(key)) return this.images.get(key);
    const entry = this.poseFor(name, pose);
    if (!entry) return null;
    const img = el("img", {alt: ""});
    img.decoding = "async";
    img.src = entry.src;
    this.images.set(key, img);
    return img;
  };

  // Whether the artwork for this character is usable at all.
  has = (name: string) => {
    return Boolean(this.character(name) && this.poseFor(name, "rest"));
  };

  // The species/role a character is, as the app already means it.
  role = (name: string) => {
    const character_ = this.character(name);
    return character_ ? character_.role || null : null;
  };
}

function createCompanionSprites(manifestUrl = "companions.manifest.json") {
  return new CompanionSpriteCatalog(manifestUrl);
}
