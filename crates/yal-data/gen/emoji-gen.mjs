// Emoji dataset generator.
//
// Sources: Unicode's emoji-test.txt (the official fully-qualified emoji
// catalogue) and the CLDR English annotations (names and keywords), both
// published under the Unicode licence. The keyword-building approach —
// CLDR keywords verbatim, plus tokenised names, unicode names and the
// subgroup treated as an alias — follows the algorithm in Vicinae's
// glyph generator (github.com/vicinaehq/vicinae, GPL-3.0), per
// docs/observations.md §6.3.
//
// Run: node emoji-gen.mjs <emoji-test.txt> <annotations.json>
//                            <annotationsDerived.json>
// Emits the Rust data table this crate ships: src/emoji/emoji_data.rs.
import { readFileSync, writeFileSync } from "node:fs";

const [, , emojiTestPath, basePath, derivedPath] = process.argv;

// --- CLDR annotations -------------------------------------------------

const VARIATION_SELECTOR_16 = /\u{FE0F}/gu;

function loadAnnotations(basePath, derivedPath) {
  const base = JSON.parse(readFileSync(basePath, "utf8")).annotations.annotations;
  const derived = JSON.parse(readFileSync(derivedPath, "utf8")).annotationsDerived.annotations;
  return { ...base, ...derived };
}

function lookupAnnotation(annotations, glyph) {
  return annotations[glyph] ?? annotations[glyph.replace(VARIATION_SELECTOR_16, "")];
}

// --- keywords ---------------------------------------------------------

const STOPWORDS = new Set(["a", "an", "and", "the", "of", "to", "for", "in", "on", "with", "as", "or", "at", "by", "from"]);

function tokenize(text) {
  return text
    .toLowerCase()
    .split(/[^\p{L}\p{N}]+/u)
    .filter((token) => token.length >= 2 && !STOPWORDS.has(token));
}

function buildKeywords(sources) {
  const keywords = [];
  const seen = new Set();
  const add = (value) => {
    const normalized = value.trim().toLowerCase();
    if (normalized && !seen.has(normalized)) {
      seen.add(normalized);
      keywords.push(normalized);
    }
  };

  for (const keyword of sources.cldr) add(keyword);

  const tokens = new Set();
  for (const source of [sources.displayName, sources.unicodeName, ...sources.aliases]) {
    for (const token of tokenize(source)) tokens.add(token);
  }
  for (const token of [...tokens].sort()) add(token);

  return keywords;
}

// --- emoji-test.txt parsing -------------------------------------------

function parseEmoji(emojiTestPath, annotations) {
  const emojis = [];
  let group = "";
  let subgroup = "";
  const isSkinTone = (cp) => cp >= 0x1f3fb && cp <= 0x1f3ff;

  for (const raw of readFileSync(emojiTestPath, "utf8").split("\n")) {
    const groupMatch = raw.match(/^# group:\s*(.+)$/);
    if (groupMatch) {
      group = groupMatch[1].trim();
      continue;
    }
    const subgroupMatch = raw.match(/^# subgroup:\s*(.+)$/);
    if (subgroupMatch) {
      subgroup = subgroupMatch[1].trim();
      continue;
    }
    if (!raw || raw.startsWith("#")) continue;

    const [codes, rest] = raw.split(";");
    if (!rest) continue;
    if (rest.split("#")[0].trim() !== "fully-qualified") continue;
    if (group === "Component") continue;

    const codepoints = codes.trim().split(/\s+/).map((hex) => parseInt(hex, 16));
    if (codepoints.some(isSkinTone)) continue;

    const glyph = String.fromCodePoint(...codepoints);
    const comment = raw.slice(raw.indexOf("#") + 1).trim();
    const testName = comment.match(/^\S+\s+E[0-9.]+\s+(.+)$/)?.[1]?.trim() ?? "";
    const annotation = lookupAnnotation(annotations, glyph);
    const name = annotation?.tts?.[0] ?? testName;

    if (!name) continue;

    emojis.push({
      glyph,
      name,
      keywords: buildKeywords({
        displayName: name,
        unicodeName: testName,
        cldr: annotation?.default ?? [],
        aliases: [subgroup.replace(/-/g, " ")],
      }),
      group,
      subgroup,
    });
  }

  return emojis;
}

// --- Rust emission ----------------------------------------------------

const rustEscape = (s) => s.replace(/\\/g, "\\\\").replace(/"/g, '\\"');

function emit(emojis) {
  // First entry wins on name collisions; the duplicate glyph joins its
  // namesake's keywords so it stays findable.
  const dataset = [];
  const byName = new Map();
  for (const entry of emojis) {
    const existing = byName.get(entry.name);
    if (existing) {
      for (const keyword of entry.keywords) {
        if (!existing.keywords.includes(keyword)) existing.keywords.push(keyword);
      }
      for (const token of tokenize(entry.name)) {
        if (!existing.keywords.includes(token)) existing.keywords.push(token);
      }
      continue;
    }
    byName.set(entry.name, entry);
    dataset.push(entry);
  }

  const rows = dataset.map((entry) => {
    const kw = entry.keywords.map((k) => `"${rustEscape(k)}"`).join(", ");
    return `    ("${rustEscape(entry.glyph)}", "${rustEscape(entry.name)}", &[${kw}]),`;
  });

  return `// GENERATED FILE — do not edit by hand.
//
// Regenerate with: node gen/emoji-gen.mjs <emoji-test.txt> <annotations.json>
//                   <annotationsDerived.json> > src/emoji_data.rs
//
// Sources: Unicode emoji-test.txt (the official fully-qualified emoji
// catalogue) and the Unicode CLDR English annotations, both under the
// Unicode licence (unicode.org/license). The keyword-building approach
// follows the generator in Vicinae (github.com/vicinaehq/vicinae,
// GPL-3.0), per docs/observations.md §6.3.
//
// Skin-tone sequences are dropped; the base emoji stays, findable by its
// name — tone companions can be composed later if the picker wants them.

/// The full dataset: character, name, lowercased search keywords.
${"pub"} static EMOJI: &[(&str, &str, &[&str])] = &[
${rows.join("\n")}
];
`;
}

const annotations = loadAnnotations(basePath, derivedPath);
const emojis = parseEmoji(emojiTestPath, annotations);
const source = emit(emojis);
const out = process.argv.includes("--stdout") ? null : "src/emoji/emoji_data.rs";
if (out) {
  writeFileSync(out, source, "utf8");
  console.log(`wrote ${out}: ${emojis.length} parsed, emitted`);
} else {
  process.stdout.write(source);
}
