#!/usr/bin/env node
// Deterministic, offline release metadata and documentation checks.
import assert from 'node:assert/strict';
import { readFileSync, existsSync, readdirSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const read = (path) => readFileSync(resolve(root, path), 'utf8');
const json = (path) => JSON.parse(read(path));
const pkg = json('package.json');
const version = pkg.version;
assert.match(version, /^\d+\.\d+\.\d+$/);
const tag = process.argv[2];
if (tag) assert.equal(tag, `v${version}`, 'Tag and source version differ');
assert.equal(pkg.license, 'GPL-3.0-only');
assert.equal(json('package-lock.json').version, version);
assert.equal(json('package-lock.json').packages[''].version, version);
assert.equal(json('src-tauri/tauri.conf.json').version, version);
assert.equal(json('src-tauri/tauri.conf.json').bundle.license, pkg.license);
assert.equal(json('.codex-plugin/plugin.json').version, version);
assert.equal(json('.codex-plugin/plugin.json').license, pkg.license);
const marketplace = json('.claude-plugin/marketplace.json');
assert.equal(marketplace.metadata.version, version);
assert.equal(marketplace.plugins.find((item) => item.name === 'zxyssh').version, version);
const workspace = read('Cargo.toml').split('[workspace.package]')[1]?.split('\n[')[0];
assert.match(workspace ?? '', new RegExp(`version = "${version.replaceAll('.', '\\.')}"`));
assert.match(workspace ?? '', /license = "GPL-3.0-only"/);
for (const path of ['src-tauri/Cargo.toml', 'cli/Cargo.toml', 'plugins/Cargo.toml']) {
  assert.match(read(path), /license\.workspace = true/);
}
const lock = read('Cargo.lock');
for (const name of ['zxyssh-desktop', 'zxyssh-plugins', 'zxyssh']) {
  const block = lock.split('[[package]]').find((entry) => entry.includes(`name = "${name}"`));
  assert.match(block ?? '', new RegExp(`version = "${version.replaceAll('.', '\\.')}"`), `Cargo.lock ${name}`);
}
assert.match(read('LICENSE'), /GNU GENERAL PUBLIC LICENSE\s+Version 3, 29 June 2007/);
assert.match(read('licenses/legacy-MIT.txt'), /MIT License/);
assert.match(read('NOTICE'), /GPL-3.0-only/);
assert(!pkg.dependencies?.gsap && !pkg.dependencies?.['@gsap/react']);
assert(!pkg.scripts?.['build:msi'], 'Windows installer build script must be removed');
const ciWorkflow = read('.github/workflows/ci.yml');
const releaseWorkflow = read('.github/workflows/release.yml');
assert(!/windows-latest|macos-latest|x86_64-pc-windows-msvc|apple-darwin|APPLE_CERTIFICATE|MACOSX_DEPLOYMENT_TARGET/.test(ciWorkflow + releaseWorkflow), 'Release/CI must not build or sign macOS/Windows artifacts');
assert.match(read('scripts/prepare-sidecar.mjs'), /Linux targets only/);
assert.equal(json('src-tauri/tauri.conf.json').bundle.targets, 'all');
assert.match(read('README.md'), /zxySSH/);
assert.match(read('README.zh-CN.md'), /zxySSH/);
assert.match(read('README.ja.md'), /zxySSH/);
assert.equal(pkg.name, 'zxyssh');
assert.equal(json('src-tauri/tauri.conf.json').productName, 'zxySSH');
assert.equal(json('src-tauri/tauri.conf.json').identifier, 'com.zxyssh.desktop');
assert.deepEqual(json('src-tauri/capabilities/default.json').platforms, ['linux', 'macOS', 'windows']);
assert.equal(json('src-tauri/tauri.conf.json').mainBinaryName, 'zxyssh-desktop');
assert.equal(read('index.html').includes('<title>zxySSH</title>'), true);
const canonical = read('skills/zxyssh/SKILL.md');
const ids = readdirSync(resolve(root, 'plugins/builtin'), { withFileTypes: true })
  .filter((entry) => entry.isDirectory()).map((entry) => entry.name).sort();
for (const directory of ['skills/zxyssh', '.claude/skills/zxyssh', '.codex/skills/zxyssh']) {
  assert.equal(read(`${directory}/SKILL.md`), canonical, `Skill drift: ${directory}`);
  const actual = readdirSync(resolve(root, directory, 'references')).filter((name) => name.endsWith('.md')).map((name) => name.slice(0, -3)).sort();
  assert.deepEqual(actual, ids, `Reference catalog drift: ${directory}`);
  for (const id of ids) assert(canonical.includes(`references/${id}.md`));
}
for (const path of ['README.md', 'README.zh-CN.md', 'README.ja.md', 'CONTRIBUTING.md', 'docs/RELEASING.md', 'docs/AGENT_COLLABORATION.md']) {
  const text = read(path);
  for (const match of text.matchAll(/\]\(([^\s)]+)\)/g)) {
    const target = match[1].split('#')[0];
    if (!target || /^[a-z][a-z0-9+.-]*:/i.test(target)) continue;
    assert(existsSync(resolve(root, dirname(path), target)), `${path}: missing ${target}`);
  }
}
console.log(`Release metadata, license, README links and ${ids.length} plugin references agree at ${version}.`);
