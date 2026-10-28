import assert from 'node:assert/strict'
import { execFileSync, spawnSync } from 'node:child_process'
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { test } from 'node:test'
import { nextVersion } from '../scripts/release.mjs'

const command = fileURLToPath(new URL('../scripts/release.mjs', import.meta.url))
function fixture(t: { after: (fn: () => void) => void }) {
  const temp = mkdtempSync(join(tmpdir(), 'fonicca-release-'))
  t.after(() => rmSync(temp, { recursive: true, force: true }))
  const root = join(temp, 'repo')
  mkdirSync(join(root, 'src-tauri'), { recursive: true })
  const git = (...args: string[]) => execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim()
  const read = (path: string) => readFileSync(join(root, path), 'utf8')
  const write = (path: string, text: string) => writeFileSync(join(root, path), text)
  write('package.json', '{"name":"fonicca","version":"0.1.1"}\n')
  write('src-tauri/tauri.conf.json', '{"version":"0.1.1"}\n')
  write('src-tauri/Cargo.toml', '[package]\nname = "fonicca"\nversion = "0.1.1"\n[dependencies]\nother = "0.1.1"\n')
  write('src-tauri/Cargo.lock', 'version = 4\n[[package]]\nname = "other"\nversion = "0.1.1"\n[[package]]\nname = "fonicca"\nversion = "0.1.1"\n')
  write('CHANGELOG.md', '# Changelog\n\n## [0.1.1] - 2026-10-27\n\n- Previous release.\n')
  git('init', '-b', 'main')
  git('config', 'user.name', 'Release Test')
  git('config', 'user.email', 'release@example.test')
  git('config', 'commit.gpgsign', 'false')
  git('config', 'tag.gpgsign', 'false')
  git('add', '.')
  git('commit', '-m', 'Previous release')
  git('tag', 'v0.1.1')
  write('fix.txt', 'signed app\n')
  git('add', '.')
  git('commit', '-m', 'Fix macOS bundle signature')
  const manager = join(temp, 'manager.mjs')
  const checks = join(temp, 'checks.log')
  writeFileSync(manager, `import {appendFileSync} from 'node:fs'; appendFileSync(${JSON.stringify(checks)},process.argv[2]+'\\n'); if(process.env.FAIL_BUILD && process.argv[2]==='build') process.exit(7);`)
  const run = (args: string[], env: Record<string, string> = {}) => spawnSync(process.execPath, [command, ...args], { cwd: root, encoding: 'utf8', env: { ...process.env, npm_execpath: manager, ...env } })
  return { temp, root, git, read, write, run, checks }
}

test('stable version bumps reset lower parts and reject downgrades or malformed versions', () => {
  assert.equal(nextVersion('1.2.3', 'patch'), '1.2.4')
  assert.equal(nextVersion('1.2.3', 'minor'), '1.3.0')
  assert.equal(nextVersion('1.2.3', 'major'), '2.0.0')
  assert.equal(nextVersion('1.2.3', '1.2.9'), '1.2.9')
  for (const value of ['1.2.3', '1.1.99', '0.99.99', '01.2.4', '1.2.4-beta.1', 'invalid'])
    assert.throws(() => nextVersion('1.2.3', value))
})

test('dry run previews commits without changing files, tags or running checks', t => {
  const f = fixture(t)
  const head = f.git('rev-parse', 'HEAD')
  const result = f.run(['patch', '--dry-run'])
  assert.equal(result.status, 0, result.stderr)
  assert.match(result.stdout, /Fix macOS bundle signature/)
  assert.match(result.stdout, /0\.1\.1 → 0\.1\.2/)
  assert.equal(f.git('status', '--porcelain'), '')
  assert.equal(f.git('rev-parse', 'HEAD'), head)
  assert.equal(f.git('tag', '--list', 'v0.1.2'), '')
  assert.throws(() => readFileSync(f.checks))
})

test('release synchronizes app versions, preserves dependency versions and history, and tags its commit', t => {
  const f = fixture(t)
  const result = f.run(['minor'])
  assert.equal(result.status, 0, result.stderr)
  assert.equal(JSON.parse(f.read('package.json')).version, '0.2.0')
  assert.equal(JSON.parse(f.read('src-tauri/tauri.conf.json')).version, '0.2.0')
  assert.match(f.read('src-tauri/Cargo.toml'), /version = "0.2.0"/)
  assert.match(f.read('src-tauri/Cargo.toml'), /other = "0.1.1"/)
  assert.match(f.read('src-tauri/Cargo.lock'), /name = "other"\nversion = "0.1.1"/)
  assert.match(f.read('src-tauri/Cargo.lock'), /name = "fonicca"\nversion = "0.2.0"/)
  assert.match(f.read('CHANGELOG.md'), /## \[0.2.0\][\s\S]*Fix macOS[\s\S]*Previous release/)
  assert.match(f.read('docs/releases/v0.2.0.md'), /Fix macOS bundle signature/)
  assert.equal(f.git('rev-parse', 'v0.2.0^{}'), f.git('rev-parse', 'HEAD'))
  assert.equal(f.git('status', '--porcelain'), '')
  assert.equal(readFileSync(f.checks, 'utf8'), 'test\nbuild\n')
})

test('dirty trees and failed checks leave the version and release tags unchanged', t => {
  const f = fixture(t)
  f.write('pending.txt', 'uncommitted')
  assert.notEqual(f.run(['patch']).status, 0)
  rmSync(join(f.root, 'pending.txt'))
  const failed = f.run(['patch'], { FAIL_BUILD: '1' })
  assert.notEqual(failed.status, 0)
  assert.equal(JSON.parse(f.read('package.json')).version, '0.1.1')
  assert.equal(f.git('tag', '--list', 'v0.1.2'), '')
  assert.equal(f.git('status', '--porcelain'), '')
})

test('push sends the release commit and tag together to origin', t => {
  const f = fixture(t)
  const remote = join(f.temp, 'origin.git')
  execFileSync('git', ['init', '--bare', remote])
  f.git('remote', 'add', 'origin', remote)
  const result = f.run(['patch', '--push'])
  assert.equal(result.status, 0, result.stderr)
  const remoteGit = (...args: string[]) => execFileSync('git', ['--git-dir', remote, ...args], { encoding: 'utf8' }).trim()
  assert.equal(remoteGit('rev-parse', 'refs/heads/main'), f.git('rev-parse', 'HEAD'))
  assert.equal(remoteGit('rev-parse', 'v0.1.2^{}'), f.git('rev-parse', 'HEAD'))
})
