import { appendFileSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const number = '(?:0|[1-9]\\d*)'
const identifier = '(?:0|[1-9]\\d*|\\d*[A-Za-z-][0-9A-Za-z-]*)'
const semver = new RegExp(`^${number}\\.${number}\\.${number}(?:-${identifier}(?:\\.${identifier})*)?(?:\\+[0-9A-Za-z-]+(?:\\.[0-9A-Za-z-]+)*)?$`)
const field = (text, name) => text.match(new RegExp(`^${name}\\s*=\\s*"([^"\\n]+)"`, 'm'))?.[1]

export function verifyRelease(files, tag = '') {
  const manifest = files.cargoToml.split(/^\[/m).find(section => section.startsWith('package]')) ?? ''
  const name = field(manifest, 'name')
  if (!name) throw new Error('Cannot find [package].name in src-tauri/Cargo.toml')
  const lock = files.cargoLock.split('[[package]]').find(section => field(section, 'name') === name) ?? ''
  const versions = {
    'package.json': files.packageJson.version,
    'src-tauri/Cargo.toml': field(manifest, 'version'),
    'src-tauri/Cargo.lock': field(lock, 'version'),
    'src-tauri/tauri.conf.json': files.tauriJson.version,
  }
  const version = versions['package.json']
  if (typeof version !== 'string' || version.trim() !== version || !semver.test(version)) throw new Error('App version must be valid SemVer, e.g. 0.1.1 or 0.2.0-beta.1')
  for (const [file, actual] of Object.entries(versions)) {
    if (actual !== version) throw new Error(`${file}: expected version ${version}, found ${actual ?? 'missing'}`)
  }
  if (tag && tag !== `v${version}`) throw new Error(`Tag ${tag} does not match app version ${version}; expected v${version}`)
  return { version, prerelease: version.split('+')[0].includes('-') }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const read = path => readFileSync(path, 'utf8')
    const result = verifyRelease({
      packageJson: JSON.parse(read('package.json')),
      cargoToml: read('src-tauri/Cargo.toml'),
      cargoLock: read('src-tauri/Cargo.lock'),
      tauriJson: JSON.parse(read('src-tauri/tauri.conf.json')),
    }, process.argv[2] ?? process.env.RELEASE_TAG ?? '')
    console.log(`Release version: ${result.version}`)
    if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `version=${result.version}\nprerelease=${result.prerelease}\n`)
  }
  catch (error) {
    console.error(error.message)
    process.exitCode = 1
  }
}
