import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { checkVersions, nextVersion, setVersion } from '../version.mjs';

test('release numbering carries at nine and always increases', () => {
  assert.equal(nextVersion('1.7.4'), '1.7.5');
  assert.equal(nextVersion('1.7.9'), '1.8.0');
  assert.equal(nextVersion('1.9.9'), '2.0.0');
  assert.equal(nextVersion('9.9.9'), '10.0.0');
  assert.throws(() => nextVersion('1.7.10'));
  assert.throws(() => nextVersion('v1.7.4'));
});

test('build version synchronizes metadata and rejects a stale installer version', () => {
  const root = mkdtempSync(resolve(tmpdir(), 'csw-release-test-'));
  try {
    mkdirSync(resolve(root, 'codex-switch-app/src-tauri'), { recursive: true });
    for (const [path, value] of [
      ['package.json', { name: 'codex-switch-app', version: '1.7.4' }],
      ['package-lock.json', { version: '1.7.4', packages: { '': { name: 'codex-switch-app', version: '1.7.4' } } }],
      ['src-tauri/tauri.conf.json', { version: '1.7.3' }]
    ]) writeFileSync(resolve(root, 'codex-switch-app', path), JSON.stringify(value));
    writeFileSync(resolve(root, 'codex-switch-app/src-tauri/Cargo.toml'), '[package]\nname = "codex-switch-app"\nversion = "1.7.4"\n');
    writeFileSync(resolve(root, 'codex-switch-app/src-tauri/Cargo.lock'), '[[package]]\nname = "codex-switch-app"\nversion = "1.7.4"\n');
    assert.throws(() => checkVersions(root), /tauri.conf/);
    assert.equal(setVersion('1.7.5', root), '1.7.5');
    assert.equal(checkVersions(root), '1.7.5');
    assert.match(readFileSync(resolve(root, 'codex-switch-app/src-tauri/Cargo.lock'), 'utf8'), /version = "1.7.5"/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});
