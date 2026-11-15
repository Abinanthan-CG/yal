// Prints the enumeration values a Raycast native addon exports.
//
//   node.exe addon-enums.js <path to .node file> [name ...]

const path = process.argv[2];
const addon = require(path);
const wanted = process.argv.slice(3);

for (const key of Object.keys(addon).sort()) {
  if (wanted.length > 0 && !wanted.includes(key)) continue;
  const value = addon[key];
  if (!value || typeof value !== 'object') continue;
  const members = Object.keys(value);
  if (members.length === 0) continue;
  console.log(`\n${key}  (${members.length})`);
  for (const member of members) {
    const item = value[member];
    console.log(
      `    ${String(member).padEnd(34)} ${typeof item === 'object' ? JSON.stringify(item) : String(item)}`
    );
  }
}
