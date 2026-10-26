import assert from 'node:assert/strict'
import { test } from 'node:test'
import { verifyRelease } from '../scripts/verify-release.mjs'

function files(version = '0.1.1') {
  return {
    packageJson: { version }, tauriJson: { version },
    cargoToml: `[package]\nname = "fonicca"\nversion = "${version}"\n[dependencies]\nfoo = "2"\n`,
    cargoLock: `version = 4\n[[package]]\nname = "dependency"\nversion = "9.9.9"\n[[package]]\nname = "fonicca"\nversion = "${version}"\n`,
  }
}

test('release tag must match all app versions, independently of dependency versions', () => {
  assert.deepEqual(verifyRelease(files(), 'v0.1.1'), { version: '0.1.1', prerelease: false })
  assert.throws(() => verifyRelease(files(), 'v0.1.2'), /does not match/)
  for (const mutation of [
    { packageJson: { version: '0.1.2' } }, { tauriJson: { version: '0.1.2' } },
    { cargoToml: files('0.1.2').cargoToml }, { cargoLock: files('0.1.2').cargoLock },
  ]) assert.throws(() => verifyRelease({ ...files(), ...mutation }), /expected version/)
  assert.throws(() => verifyRelease({ ...files(), cargoLock: '' }), /missing/)
})

test('prerelease detection and SemVer validation reject malformed release versions', () => {
  assert.deepEqual(verifyRelease(files('0.2.0-beta.1'), 'v0.2.0-beta.1'), { version: '0.2.0-beta.1', prerelease: true })
  assert.equal(verifyRelease(files('0.2.0+build-1')).prerelease, false)
  for (const version of ['01.2.3', '1.2', '1.2.3-beta.01', 'v1.2.3', '1.2.3\n'])
    assert.throws(() => verifyRelease(files(version)), /valid SemVer/)
})
