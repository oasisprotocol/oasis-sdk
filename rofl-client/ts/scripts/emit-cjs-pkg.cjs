// scripts/emit-cjs-pkg.cjs
const fs = require('node:fs');
const path = require('node:path');

const dir = path.join(__dirname, '..', 'dist', 'cjs');
fs.mkdirSync(dir, {recursive: true});
fs.writeFileSync(path.join(dir, 'package.json'), JSON.stringify({type: 'commonjs'}, null, 2));
