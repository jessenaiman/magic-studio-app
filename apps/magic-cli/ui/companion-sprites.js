class CompanionSpriteCatalog {
  images = new Map;
  manifest = { version: 0, characters: {} };
  manifestUrl;
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
    } catch (_) {}
    this.ready = true;
    return this.manifest;
  };
  character = (name) => {
    return this.manifest.characters && this.manifest.characters[name] ? this.manifest.characters[name] : null;
  };
  poseFor = (name, pose) => {
    const character_ = this.character(name);
    if (!character_ || !character_.poses)
      return null;
    const wanted = character_.poses[pose];
    const fallback = character_.poses.rest;
    const src = wanted || fallback;
    return src ? { src, pose: wanted ? pose : "rest" } : null;
  };
  image = (name, pose) => {
    const key = `${name}:${pose}`;
    if (this.images.has(key))
      return this.images.get(key);
    const entry = this.poseFor(name, pose);
    if (!entry)
      return null;
    const img = el("img", { alt: "" });
    img.decoding = "async";
    img.src = entry.src;
    this.images.set(key, img);
    return img;
  };
  has = (name) => {
    return Boolean(this.character(name) && this.poseFor(name, "rest"));
  };
  role = (name) => {
    const character_ = this.character(name);
    return character_ ? character_.role || null : null;
  };
}
function createCompanionSprites(manifestUrl = "companions.manifest.json") {
  return new CompanionSpriteCatalog(manifestUrl);
}
