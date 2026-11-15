// Describes the API surface of a Raycast native addon.
//
//   node.exe addon-api.js <path to .node file>

// For each exported class it lists the instance methods (from the prototype)
// and the members of the exported enumerations. This is the vocabulary the
// product is written in - the names of its concepts - which is what a port
// needs to know.

const path = process.argv[2];
const addon = require(path);

function describe(name, value) {
  if (typeof value !== 'function') return;
  const proto = value.prototype;
  if (!proto) return;
  const members = Object.getOwnPropertyNames(proto).filter(
    (member) => member !== 'constructor'
  );
  if (members.length === 0) return;
  console.log(`\n${name}`);
  for (const member of members) {
    const descriptor = Object.getOwnPropertyDescriptor(proto, member);
    const kind = descriptor && typeof descriptor.value === 'function'
      ? `fn/${descriptor.value.length}`
      : 'getter';
    console.log(`    ${member.padEnd(40)} ${kind}`);
  }
}

function describeEnum(name, value) {
  if (!value || typeof value !== 'object') return;
  const members = Object.keys(value).filter((key) => isNaN(Number(key)));
  if (members.length === 0) return;
  console.log(`\n${name}  (${members.length} members)`);
  for (const member of members) {
    const item = value[member];
    console.log(`    ${member.padEnd(40)} ${JSON.stringify(item)}`);
  }
}

for (const key of Object.keys(addon).sort()) {
  const value = addon[key];
  if (typeof value === 'function') {
    describe(key, value);
  } else if (value && typeof value === 'object') {
    describeEnum(key, value);
  }
}
