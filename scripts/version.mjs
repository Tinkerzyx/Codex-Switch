import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { resolve } from 'node:path';

export const repositoryRoot = fileURLToPath(new URL('../', import.meta.url));
const json = (root, path) => JSON.parse(readFileSync(resolve(root, path), 'utf8'));

export function nextVersion(version) {
  if (!/^\d+\.[0-9]\.[0-9]$/.test(version)) throw new Error(`Expected decimal release version, got ${version}`);
  let [major, minor, patch] = version.split('.').map(Number);
  patch += 1;
  if (patch === 10) { patch = 0; minor += 1; }
  if (minor === 10) { minor = 0; major += 1; }
  return `${major}.${minor}.${patch}`;
}

export function readVersion(root = repositoryRoot) {
  return json(root, 'codex-switch-app/package.json').version;
}

export function checkVersions(root = repositoryRoot) {
  const version = readVersion(root);
  if (!/^\d+\.[0-9]\.[0-9]$/.test(version)) throw new Error(`Invalid release version: ${version}`);
  const lock = json(root, 'codex-switch-app/package-lock.json');
  const tauri = json(root, 'codex-switch-app/src-tauri/tauri.conf.json');
  const cargo = readFileSync(resolve(root, 'codex-switch-app/src-tauri/Cargo.toml'), 'utf8');
  const cargoLock = readFileSync(resolve(root, 'codex-switch-app/src-tauri/Cargo.lock'), 'utf8');
  const values = {
    'package-lock.json': lock.version,
    'package-lock root package': lock.packages[''].version,
    'tauri.conf.json': tauri.version,
    'Cargo.toml': cargo.match(/\[package\][\s\S]*?^version = "([^"]+)"/m)?.[1],
    'Cargo.lock': cargoLock.match(/name = "codex-switch-app"\r?\nversion = "([^"]+)"/)?.[1]
  };
  for (const [file, found] of Object.entries(values)) {
    if (found !== version) throw new Error(`${file}: ${found} differs from package.json: ${version}`);
  }
  return version;
}

export function setVersion(version, root = repositoryRoot) {
  if (!/^\d+\.[0-9]\.[0-9]$/.test(version)) throw new Error(`Invalid release version: ${version}`);
  const updates = new Map();
  for (const path of ['codex-switch-app/package.json', 'codex-switch-app/package-lock.json', 'codex-switch-app/src-tauri/tauri.conf.json']) {
    const value = json(root, path);
    value.version = version;
    if (value.packages?.['']) value.packages[''].version = version;
    updates.set(path, JSON.stringify(value, null, 2) + '\n');
  }
  for (const [path, pattern] of [
    ['codex-switch-app/src-tauri/Cargo.toml', /(\[package\][\s\S]*?^version = ")[^"]+(".*$)/m],
    ['codex-switch-app/src-tauri/Cargo.lock', /(name = "codex-switch-app"\r?\nversion = ")[^"]+(".*$)/m]
  ]) {
    const before = readFileSync(resolve(root, path), 'utf8');
    if (!pattern.test(before)) throw new Error(`Package version missing in ${path}`);
    updates.set(path, before.replace(pattern, (_, prefix, suffix) => prefix + version + suffix));
  }
  // Prepare all files before writing any; a partial set is rejected by the build check.
  for (const [path, content] of updates) writeFileSync(resolve(root, path), content);
  return checkVersions(root);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const action = process.argv[2] || 'check';
  try {
    const version = action === 'bump' ? setVersion(nextVersion(checkVersions()))
      : action === 'sync' ? setVersion(readVersion())
      : action === 'check' ? checkVersions() : (() => { throw new Error('Use check, sync, or bump.'); })();
    if (process.env.RELEASE_TAG && process.env.RELEASE_TAG !== `v${version}`) throw new Error(`Tag ${process.env.RELEASE_TAG} does not match v${version}`);
    console.log(version);
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
