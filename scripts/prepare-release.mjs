import { createHash } from 'node:crypto'
import { createReadStream } from 'node:fs'
import { readdir, readFile, writeFile } from 'node:fs/promises'
import { basename, join, resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

async function walk(directory) {
  const entries = await readdir(directory, { withFileTypes: true })
  const children = await Promise.all(entries.map(entry => entry.isDirectory() ? walk(join(directory, entry.name)) : [join(directory, entry.name)]))
  return children.flat()
}

/** Validate complete signed releases before creating a draft or making its feed public. */
export async function prepareRelease({ directory, version, tag, repository, date = new Date().toISOString(), signed = true }) {
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository ?? '') || tag !== `v${version}`)
    throw new Error('Expected a repository and matching release tag')
  const all = await walk(directory)
  const files = all.filter(path => (signed ? /\.(dmg|exe|msi|deb|AppImage|app\.tar\.gz)(\.sig)?$/ : /\.(dmg|exe|msi|deb|AppImage)$/).test(path)).sort()
  const names = files.map(path => basename(path))
  if (new Set(names).size !== names.length || names.some(name => /[\r\n#]/.test(name)))
    throw new Error('Release asset names must be unique and safe for upload')
  function only(suffix) {
    const matches = files.filter(path => path.endsWith(suffix))
    if (matches.length !== 1) throw new Error(`Expected exactly one ${suffix} artifact; found ${matches.length}`)
    return matches[0]
  }
  for (const suffix of ['.dmg', '.exe', '.msi', '.deb', '.AppImage']) only(suffix)
  const downloadBase = `https://github.com/${repository}/releases/download/${encodeURIComponent(tag)}`
  const platforms = {}
  if (signed) for (const [platform, suffix] of [
    ['darwin-aarch64', '.app.tar.gz'], ['windows-x86_64', '.exe'], ['linux-x86_64', '.AppImage'],
  ]) {
    const artifact = only(suffix)
    const signaturePath = `${artifact}.sig`
    if (!files.includes(signaturePath)) throw new Error(`Missing signature for ${basename(artifact)}`)
    const signature = (await readFile(signaturePath, 'utf8')).trim()
    if (!signature || !Buffer.from(signature, 'base64').toString().startsWith('untrusted comment:'))
      throw new Error(`Invalid signature file for ${basename(artifact)}`)
    platforms[platform] = { signature, url: `${downloadBase}/${encodeURIComponent(basename(artifact))}` }
  }
  const manifest = {
    version, notes: `https://github.com/${repository}/releases/tag/${encodeURIComponent(tag)}`,
    pub_date: date, platforms,
  }
  const manifestPath = join(directory, 'latest.json')
  if (signed) {
    await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`)
    files.push(manifestPath)
  }
  const lines = []
  for (const path of files) {
    const hash = createHash('sha256')
    for await (const chunk of createReadStream(path)) hash.update(chunk)
    lines.push(`${hash.digest('hex')}  ${basename(path)}\n`)
  }
  const sums = join(directory, 'SHA256SUMS.txt')
  await writeFile(sums, lines.join(''))
  return { manifest, assets: [...files, sums] }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const { assets } = await prepareRelease({
    directory: 'installers', version: process.env.RELEASE_VERSION,
    tag: process.env.RELEASE_TAG, repository: process.env.GH_REPO, signed: process.env.UPDATES_ENABLED === 'true',
  })
  await writeFile('release-assets.txt', assets.map(path => `${path}\n`).join(''))
}
