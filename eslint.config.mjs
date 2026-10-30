// UI normalization for `magic-cli audit --tool eslint`. CSS is component code:
// components never style elements inline. A look belongs in global.css or a view
// stylesheet, reached through a class or data-look; values come from :root tokens.
const inline = "No inline styles: give the element a class or data-look and style it in CSS.";

export default [
  { ignores: ["**/vendor/**", "**/node_modules/**", "target/**", "references/**"] },
  {
    files: ["apps/src-tauri/ui/**/*.js"],
    languageOptions: { ecmaVersion: "latest", sourceType: "script" },
    rules: {
      "no-restricted-syntax": ["error",
        { selector: "MemberExpression[object.property.name='style'][property.name=/^(setProperty|removeProperty|cssText)$/]", message: inline },
        { selector: "AssignmentExpression > MemberExpression.left[object.property.name='style']", message: inline },
        { selector: "CallExpression[callee.property.name='setAttribute'][arguments.0.value='style']", message: inline },
        { selector: "CallExpression[callee.name='el'] > ObjectExpression > Property[key.name='style']", message: inline },
      ],
    },
  },
];
