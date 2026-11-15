// Reads the shape of Raycast's stored models through its own data layer.
//
//   set RAYCAST_DB_KEY=<key> && node.exe data-model.js <data folder> <addon path>
//
// This works on a COPY of the user's data folder, never the live one, so the
// running application is not disturbed and nothing is migrated in place.
//
// Only structure is reported: field names, types and counts. Values that are
// the user's own content (names, titles, clipboard text) are not printed.

const path = require('path');

const dataFolder = process.argv[2];
const addonPath = process.argv[3];
const key = process.env.RAYCAST_DB_KEY;

if (!key) {
  console.error('set RAYCAST_DB_KEY first');
  process.exit(1);
}

const { DatabaseClient } = require(addonPath);

function shape(value, depth = 0) {
  if (value === null || value === undefined) return 'null';
  if (Array.isArray(value)) {
    if (value.length === 0) return 'array(0)';
    return `array(${value.length}) of ${shape(value[0], depth + 1)}`;
  }
  if (typeof value === 'object') {
    return `{${Object.keys(value).join(', ')}}`;
  }
  return typeof value;
}

function report(label, value) {
  console.log(`\n### ${label}`);
  console.log(`    ${shape(value)}`);
  if (value && typeof value === 'object' && !Array.isArray(value)) {
    console.log(`    own keys: ${JSON.stringify(Object.keys(value))}`);
    console.log(`    proto:    ${Object.getOwnPropertyNames(Object.getPrototypeOf(value)).join(', ')}`);
    let text;
    try {
      text = JSON.stringify(value);
    } catch (error) {
      text = `(not serialisable: ${error.message})`;
    }
    if (text) {
      console.log(`    json:     ${text.slice(0, 900)}`);
    }
  }
  if (Array.isArray(value) && value.length > 0 && typeof value[0] === 'object') {
    const sample = value[0];
    console.log(`    fields: ${JSON.stringify(sample)}`);
  }
}

const noopLogger = () => {};
const client = new DatabaseClient(dataFolder, key, noopLogger, undefined);

console.log(`host key length:      ${String(client.hostEncryptionKey).length}`);
console.log(`effective key length: ${String(client.effectiveEncryptionKey).length}`);

try {
  const status = client.getDatabaseStatus();
  report('getDatabaseStatus()', status);
} catch (error) {
  console.log(`getDatabaseStatus failed: ${error.message}`);
}

const probes = [
  ['frecency.getRecords()', () => client.frecency.getRecords()],
  ['frecency.getAll()', () => client.frecency.getAll()],
  ['settings.allThemes()', () => client.settings.allThemes()],
  ['settings.allWindowModes()', () => client.settings.allWindowModes()],
  ['quicklinks.list()', () => client.quicklinks.list()],
  ['snippets.list()', () => client.snippets.list()],
  ['notes.list()', () => client.notes.list()],
  ['clipboard.getAllTags()', () => client.clipboard.getAllTags()],
];

async function main() {
  for (const [label, probe] of probes) {
    try {
      const value = await probe();
      report(label, value);
    } catch (error) {
      console.log(`\n### ${label}\n    failed: ${error.message}`);
    }
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
