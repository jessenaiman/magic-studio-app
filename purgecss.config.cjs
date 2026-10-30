// Dead-style check for `magic-cli audit --tool purgecss`: selectors in our
// stylesheets that no component (JS) or page (HTML) can produce any more.
// Components write data attributes as `dataset.fooBar`; the extractor also
// yields `data-foo-bar` so attribute selectors set from code are not dead.
const dataset = (name) => "data-" + name.replace(/[A-Z]/g, (c) => "-" + c.toLowerCase());

module.exports = {
  content: ["apps/src-tauri/ui/**/*.html", "apps/src-tauri/ui/**/*.js"],
  css: ["apps/src-tauri/ui/*.css"],
  skippedContentGlobs: ["**/vendor/**", "**/node_modules/**"],
  rejected: true,
  defaultExtractor: (content) => [
    ...(content.match(/[\w-/:%.]+(?<!:)/g) || []),
    ...[...content.matchAll(/dataset\.(\w+)/g)].map((m) => dataset(m[1])),
  ],
};
