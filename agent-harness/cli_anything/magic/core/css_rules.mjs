// Every rule and @keyframes of the given stylesheets as JSON lines, parsed by
// PostCSS (Stylelint's parser), for audit's stylesheet-concern report.
// Run: node css_rules.mjs <postcss module path> <file.css>...
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";

const [postcssPath, ...files] = process.argv.slice(2);
const postcss = createRequire(postcssPath)(postcssPath);

for (const file of files) {
  postcss.parse(readFileSync(file, "utf8"), { from: file }).walk((node) => {
    const keyframes = node.type === "atrule" && /keyframes$/.test(node.name);
    if (node.type !== "rule" && !keyframes) return;
    if (node.parent?.type === "atrule" && /keyframes$/.test(node.parent.name)) return;
    const props = node.nodes?.filter((n) => n.type === "decl").map((n) => n.prop) ?? [];
    console.log(JSON.stringify({
      file,
      line: node.source.start.line,
      selector: keyframes ? `@keyframes ${node.params}` : node.selector,
      props,
    }));
  });
}
