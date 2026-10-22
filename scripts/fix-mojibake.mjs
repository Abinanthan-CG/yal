import { readdirSync, readFileSync, writeFileSync, statSync } from "node:fs";
import { join } from "node:path";

// Windows-1252 differs from Latin-1 only in 0x80..0x9F.
const HIGH = [
  0x20ac, 0x0081, 0x201a, 0x0192, 0x201e, 0x2026, 0x2020, 0x2021,
  0x02c6, 0x2030, 0x0160, 0x2039, 0x0152, 0x008d, 0x017d, 0x008f,
  0x0090, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014,
  0x02dc, 0x2122, 0x0161, 0x203a, 0x0153, 0x009d, 0x017e, 0x0178,
];

const HIGH_SET = new Set(HIGH);

const isHigh = (code) =>
  (code >= 0x80 && code <= 0xff) || HIGH_SET.has(code);

const isStarter = (code) =>
  code === 0xc2 || code === 0xc3 || code === 0xc5 || code === 0xe2 ||
  code === 0xe3 || code === 0xd0 || code === 0xd1;

function bytesOf(ch) {
  const code = ch.codePointAt(0);
  if (code <= 0xff) return code;
  const index = HIGH.indexOf(code);
  return index >= 0 ? 0x80 + index : null;
}

/**
 * Repair text that was read as Windows-1252 and written back as UTF-8.
 *
 * A mojibake run starts with a character that can only be produced by that
 * round trip and continues with the bytes that followed it. The run is
 * reversed by mapping its characters back to bytes and reading those bytes
 * as UTF-8.
 */
export function repair(text) {
  const characters = [...text];
  let output = "";
  let index = 0;
  let repairs = 0;

  while (index < characters.length) {
    const ch = characters[index];
    const code = ch.codePointAt(0);

    if (!isStarter(code)) {
      output += ch;
      index += 1;
      continue;
    }

    // Take the longest run of high characters that decodes as valid UTF-8.
    let best = null;
    for (let length = 4; length >= 2; length -= 1) {
      const run = characters.slice(index, index + length);
      if (run.length < 2) continue;
      if (!run.every((c) => isHigh(c.codePointAt(0)))) continue;
      const bytes = run.map((c) => bytesOf(c));
      if (bytes.some((b) => b === null)) continue;
      const decoded = Buffer.from(bytes).toString("utf8");
      if (decoded.includes("\uFFFD")) continue;
      // The repair must not itself be mojibake.
      if ([...decoded].some((c) => isStarter(c.codePointAt(0)))) continue;
      best = { length, decoded };
      break;
    }

    if (best) {
      output += best.decoded;
      index += best.length;
      repairs += 1;
    } else {
      output += ch;
      index += 1;
    }
  }

  return { text: output, repairs };
}

function scan(directory, results) {
  for (const entry of readdirSync(directory)) {
    const path = join(directory, entry);
    if (statSync(path).isDirectory()) {
      scan(path, results);
      continue;
    }
    if (!/\.(rs|toml|md)$/.test(path)) continue;
    const source = readFileSync(path, "utf8");
    const { text, repairs } = repair(source);
    if (repairs > 0) results.push({ path, text, repairs });
  }
  return results;
}

const apply = process.argv.includes("--apply");
const results = scan("crates", []);
for (const result of results) {
  console.log(`${result.path}  ${result.repairs} sequence(s)`);
  if (apply) writeFileSync(result.path, result.text, "utf8");
}
console.log(results.length === 0 ? "no mojibake found" : `${results.length} file(s) affected`);
