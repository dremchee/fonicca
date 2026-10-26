import assert from 'node:assert/strict'
import { test } from 'node:test'
import { mkdtemp, mkdir, readFile, writeFile, rm, unlink } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { prepareRelease } from '../scripts/prepare-release.mjs'
import { updaterConfig } from '../scripts/updater-config.mjs'

async function fixture() {
  const directory = await mkdtemp(join(tmpdir(), 'fonicca-release-'))
  for (const name of ['Fonicca.dmg', 'Fonicca_x64-setup.exe', 'Fonicca.msi', 'Fonicca.deb', 'Fonicca.AppImage', 'Fonicca.app.tar.gz'])
    await writeFile(join(directory, name), `binary ${name}`)
  for (const name of ['Fonicca_x64-setup.exe', 'Fonicca.AppImage', 'Fonicca.app.tar.gz'])
    await writeFile(join(directory, `${name}.sig`), Buffer.from('untrusted comment: signature fixture\nnot-a-real-signature').toString('base64'))
  return { directory, version: '0.2.0', tag: 'v0.2.0', repository: 'owner/public-downloads', date: '2026-10-26T12:00:00Z' }
}

test('signed release feed maps three platforms and includes every artifact in checksums', async () => {
  const options = await fixture()
  try {
    const { manifest, assets } = await prepareRelease(options)
    assert.equal(manifest.version, '0.2.0')
    assert.deepEqual(Object.keys(manifest.platforms), ['darwin-aarch64', 'windows-x86_64', 'linux-x86_64'])
    assert.match(manifest.platforms['windows-x86_64'].url, /owner\/public-downloads\/releases\/download\/v0.2.0\/Fonicca_x64-setup.exe$/)
    assert.equal(assets.length, 11)
    const sums = await readFile(join(options.directory, 'SHA256SUMS.txt'), 'utf8')
    assert.equal(sums.trim().split('\n').length, 10)
    assert.match(sums, /[a-f0-9]{64}  latest.json/)
  }
  finally { await rm(options.directory, { recursive: true }) }
})

test('missing signature, empty signature and duplicate basenames fail release preparation', async () => {
  const options = await fixture()
  try {
    const sig = join(options.directory, 'Fonicca.AppImage.sig')
    await writeFile(sig, '')
    await assert.rejects(prepareRelease(options), /Invalid signature/)
    await unlink(sig)
    await assert.rejects(prepareRelease(options), /Missing signature/)
    await mkdir(join(options.directory, 'extra'))
    await writeFile(join(options.directory, 'extra', 'Fonicca.dmg'), 'duplicate')
    await assert.rejects(prepareRelease(options), /unique/)
  }
  finally { await rm(options.directory, { recursive: true }) }
})

test('ordinary tag releases keep working with no updater setup or signing secrets', async () => {
  const options = await fixture()
  try {
    await unlink(join(options.directory, 'Fonicca.AppImage.sig'))
    const { assets } = await prepareRelease({ ...options, signed: false })
    assert.equal(assets.length, 6)
    assert.ok(!assets.some(path => path.endsWith('latest.json') || path.endsWith('.sig')))
  }
  finally { await rm(options.directory, { recursive: true }) }
})

test('public feed configuration keeps the source repository out of download URLs', () => {
  const config = updaterConfig('owner/public-downloads')
  assert.deepEqual(config.plugins.updater.endpoints, ['https://github.com/owner/public-downloads/releases/latest/download/latest.json'])
  assert.equal(config.bundle.createUpdaterArtifacts, true)
  assert.throws(() => updaterConfig('https://github.com/owner/repo'), /Invalid/)
})
