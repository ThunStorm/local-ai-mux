// Repository-local release guard: versions, public naming and portable document targets.
import assert from 'node:assert/strict';
import {readFileSync, existsSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {dirname, resolve} from 'node:path';
const read = path => readFileSync(path, 'utf8');
const root = JSON.parse(read('package.json'));
const desktop = JSON.parse(read('apps/desktop/package.json'));
const config = JSON.parse(read('apps/desktop/src-tauri/tauri.conf.json'));
const cargo = read('Cargo.toml');
assert.equal(desktop.version, root.version, 'frontend version');
assert.equal(config.version, root.version, 'Tauri version');
assert.equal(cargo.match(/\[workspace\.package\]\s*version\s*=\s*"([^"]+)"/)?.[1], root.version, 'workspace version');
assert.equal(config.productName, 'LocalAiMux');
assert.match(read('README.md'), /## 使用/, 'primary product guide is Chinese');
assert.match(read('docs/BRIEF.md'), /^# LocalAiMux 产品说明/, 'product brief is Chinese');
assert.equal(config.identifier, 'app.localmux.desktop', 'stable bundle identity');
assert.ok(config.app.windows.every(window => window.title === 'LocalAiMux'));
const members = JSON.parse(cargo.match(/members\s*=\s*(\[[^\]]+\])/)[1]);
for (const member of members) assert.match(read(`${member}/Cargo.toml`), /^version\.workspace = true$/m, member);
for (const pkg of read('Cargo.lock').split('[[package]]').slice(1)) {
  const name = pkg.match(/^\s*name = "([^"]+)"/m)?.[1];
  if (name?.startsWith('localmux-')) assert.equal(pkg.match(/^version = "([^"]+)"/m)?.[1], root.version, `Cargo.lock ${name}`);
}
const oldBrand = String.fromCodePoint(0x667a, 0x6c47, 0x67a2);
const docs = execFileSync('git', ['ls-files','--cached','--others','--exclude-standard','-z','*.md'], {encoding:'utf8'}).split('\0').filter(Boolean);
const errors = [];
for (const path of new Set(docs)) {
  const source = read(path);
  if (source.includes(oldBrand)) errors.push(`${path}: deprecated public name`);
  let inFence = false;
  for (const [index, line] of source.split('\n').entries()) {
    if (/^\s*(```|~~~)/.test(line)) { inFence = !inFence; continue; }
    if (inFence) continue;
    for (const match of line.matchAll(/\]\(([^)]+)\)/g)) {
      const href = match[1].replace(/^<|>$/g,'').split('#')[0];
      if (!href || /^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith('/')) continue;
      if (!existsSync(resolve(dirname(path), decodeURIComponent(href)))) errors.push(`${path}:${index+1}: missing ${href}`);
    }
  }
}
assert.deepEqual(errors, [], 'document naming and link targets');
assert.match(read('apps/desktop/src/execution-limits.ts'), /MAX_CLI_IDLE_SECONDS = 1200/);
assert.match(read('crates/adapter-sdk/src/cli.rs'), /MAX_CLI_IDLE_SECONDS: u64 = 1200/);
console.log(`LocalAiMux ${root.version}: ${members.length} Rust packages, frontend/Tauri versions, naming and ${new Set(docs).size} Markdown link targets passed`);
