import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const targets = [];
function walk(directory) {
  for (const entry of readdirSync(directory)) {
    if (entry === "target" || entry === ".git" || entry === "node_modules") continue;
    const path = join(directory, entry);
    if (statSync(path).isDirectory()) walk(path);
    else if (/\.(rs|toml|md|mjs)$/.test(path)) targets.push(path);
  }
}
walk(".");

let bad = 0;
for (const path of targets) {
  const text = readFileSync(path, "utf8");
  const starters = (text.match(/[ÂÃâÅÐÑ]/g) || []).length;
  const replacement = (text.match(/\uFFFD/g) || []).length;
  if (starters > 0 || replacement > 0) {
    console.log(`${path}  starters=${starters} replacement=${replacement}`);
    bad += 1;
  }
}

const emoji = readFileSync("crates/yal-data/src/emoji.rs", "utf8");
const grinning = emoji.match(/\("([^"]+)", "grinning"/);
console.log("emoji grinning codepoints:", [...grinning[1]].map((c) => c.codePointAt(0).toString(16)).join(" "));
console.log(bad === 0 ? "no mojibake anywhere" : `${bad} file(s) still affected`);
