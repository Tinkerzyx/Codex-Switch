import { spawnSync } from 'node:child_process';
import { mkdirSync, writeFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { repositoryRoot, checkVersions, nextVersion, setVersion } from './version.mjs';

const args = process.argv.slice(2);
const ci = args.includes('--ci') || process.env.CI === 'true';
const forwarded = args.filter(arg => arg !== '--ci');
const app = resolve(repositoryRoot, 'codex-switch-app');
const cli = resolve(app, 'node_modules/@tauri-apps/cli/tauri.js');
if (!['win32', 'darwin'].includes(process.platform)) throw new Error('Build on Windows or macOS.');
const target = forwarded[forwarded.indexOf('--target') + 1];
if (forwarded.includes('--target') && ((process.platform === 'win32') !== target.includes('windows'))) {
  throw new Error('Use a native Windows/macOS runner for each platform.');
}
const version = ci ? checkVersions() : setVersion(nextVersion(checkVersions()));
console.log(`Building Codex Switch ${version} (${process.platform}/${process.arch})`);
const result = spawnSync(process.execPath, [cli, 'build', ...forwarded, '--', '--locked'], {
  cwd: app, stdio: 'inherit', env: { ...process.env, CSW_RELEASE_VERSION: version }
});
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status || 1);
const commit = spawnSync('git', ['-c', `safe.directory=${repositoryRoot.replaceAll('\\', '/').replace(/\/$/, '')}`, 'rev-parse', 'HEAD'], { cwd: repositoryRoot, encoding: 'utf8' });
mkdirSync(resolve(repositoryRoot, 'release-output'), { recursive: true });
writeFileSync(resolve(repositoryRoot, `release-output/build-${process.platform}-${process.arch}.json`), JSON.stringify({
  version, platform: process.platform, arch: process.arch, target: forwarded.includes('--target') ? target : null,
  commit: commit.status === 0 ? commit.stdout.trim() : null
}, null, 2) + '\n');

if (!ci && process.platform === 'win32') {
  const directory = resolve(app, 'src-tauri/target/release/bundle/nsis');
  const installer = readdirSync(directory).find(name => name.includes(`_${version}_`) && name.endsWith('-setup.exe'));
  if (!installer) throw new Error(`Installer for ${version} missing in ${directory}`);
  const install = spawnSync('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File',
    resolve(repositoryRoot, 'scripts/install-windows.ps1'), '-Installer', resolve(directory, installer)], { stdio: 'inherit' });
  if (install.error) throw install.error;
  if (install.status !== 0) process.exit(install.status || 1);
}
