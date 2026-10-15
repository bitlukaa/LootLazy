const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const root = path.resolve(__dirname, '..');
const lock = JSON.parse(fs.readFileSync(path.join(root, 'third_party/asf-source.lock.json'), 'utf8').replace(/^\uFEFF/, ''));
const prefix = lock.sourceDirectory + '/';
const staged = execFileSync('git', ['ls-files', '--stage', '--', lock.sourceDirectory], { cwd: root, encoding: 'utf8' });
const actual = new Map(staged.trim().split(/\r?\n/).filter(Boolean).map(line => {
  const match = line.match(/^(\d+) ([a-f0-9]+) 0\t(.+)$/);
  if (!match || match[1] !== '100644' || !match[3].startsWith(prefix)) throw Error('Unexpected source entry: ' + line);
  return [match[3].slice(prefix.length), match[2]];
}));
if (actual.size !== lock.files.length) throw Error(`Source count mismatch: ${actual.size}/${lock.files.length}`);
for (const file of lock.files) {
  if (actual.get(file.path) !== file.gitBlob) throw Error('Upstream blob mismatch: ' + file.path);
}
console.log(`Verified ${actual.size} unmodified ASF source files at ${lock.commit}.`);
