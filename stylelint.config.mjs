// Design rules for our stylesheets, run by `magic-cli audit --tool stylelint`.
// Each rule pushes repetition into shared, inherited classes and tokens:
// classes instead of IDs, one selector per look, and colour and type taken
// from the :root tokens rather than written out again.
const rawColor = ["/#[0-9a-f]{3,8}\\b/i", "/\\b(rgb|hsl)a?\\(/"];

export default {
  ignoreFiles: ["**/vendor/**", "**/node_modules/**", "target/**", "references/**"],
  rules: {
    "selector-max-id": 0,
    "no-duplicate-selectors": true,
    "declaration-block-no-duplicate-properties": true,
    "declaration-no-important": true,
    "declaration-property-value-disallowed-list": {
      // Type comes from the --text-* scale, never a pixel token or literal.
      "font-size": ["/--size-/", "/\\d(px|rem|em)\\b/"],
      "/^(color|background|background-color|border(-[a-z]+)?-color|fill|stroke)$/": rawColor,
    },
  },
};
