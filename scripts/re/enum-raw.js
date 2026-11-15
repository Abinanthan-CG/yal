// Prints the raw shape of an exported enumeration.
//
//   node.exe enum-raw.js <path to .node file> <name>

const addon = require(process.argv[2]);
const name = process.argv[3];
const value = addon[name];

console.log(`typeof: ${typeof value}`);
console.log(`own property names: ${JSON.stringify(Object.getOwnPropertyNames(value))}`);
console.log(`own symbols: ${value ? Object.getOwnPropertySymbols(value).length : 0}`);

for (const key of Object.getOwnPropertyNames(value)) {
  const descriptor = Object.getOwnPropertyDescriptor(value, key);
  let shown;
  try {
    shown = JSON.stringify(value[key]);
  } catch (error) {
    shown = `(throws: ${error.message})`;
  }
  console.log(
    `    ${String(key).padEnd(30)} ${descriptor.get ? 'getter' : 'value'} -> ${shown}`
  );
}
