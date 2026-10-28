import { execFileSync } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { verifyRelease } from './verify-release.mjs'

const paths = ['package.json', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'src-tauri/tauri.conf.json']
const read = path => readFileSync(path, 'utf8')
const git = (...args) => execFileSync('git', args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim()

export function nextVersion(current, bump) {
  const parse = value => {
    if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(value))
      throw new Error('This command supports stable versions only (e.g. 0.1.2).')
    const parts = value.split('.').map(Number)
    if (!parts.every(Number.isSafeInteger)) throw new Error('Version numbers are too large.')
    return parts
  }
  const previous = parse(current)
  const index = ['major', 'minor', 'patch'].indexOf(bump)
  const result = index < 0 ? parse(bump) : previous.map((value, i) => i < index ? value : i === index ? value + 1 : 0)
  if (!result.every(Number.isSafeInteger) || !result.some((value, i) => value !== previous[i] && result.slice(0, i).every((part, j) => part === previous[j]) && value > previous[i]))
    throw new Error(`New version must be greater than ${current}.`)
  return result.join('.')
}

function manifests(contents) {
  return { packageJson: JSON.parse(contents[0]), cargoToml: contents[1], cargoLock: contents[2], tauriJson: JSON.parse(contents[3]) }
}

export function release(args) {
  if (args.includes('--help')) {
    console.log('Usage: pnpm release [patch|minor|major|X.Y.Z] [--dry-run] [--push]\nDefault: patch. Runs tests and build, updates versions and changelog, commits and tags.\n--dry-run previews without checks or writes. --push atomically pushes main and the tag to origin; CI creates a release draft.')
    return
  }
  const positional = args.filter(arg => !arg.startsWith('--'))
  if (positional.length > 1 || args.some(arg => arg.startsWith('--') && !['--dry-run', '--push'].includes(arg)))
    throw new Error('Unknown arguments. Run pnpm release --help.')
  if (git('rev-parse', '--show-prefix')) throw new Error('Run this command from the repository root.')
  if (git('branch', '--show-current') !== 'main') throw new Error('Switch to main before preparing a release.')
  if (git('status', '--porcelain', '--untracked-files=all')) throw new Error('Commit or stash pending changes before preparing a release.')
  const before = paths.map(read)
  const { version: current } = verifyRelease(manifests(before))
  const version = nextVersion(current, positional[0] ?? 'patch')
  const tag = `v${version}`
  if (git('tag', '--list', tag)) throw new Error(`Tag ${tag} already exists.`)
  git('merge-base', '--is-ancestor', `refs/tags/v${current}`, 'HEAD')
  const commits = git('log', '--reverse', '--no-merges', '--format=%s (%h)', `v${current}..HEAD`).split('\n').filter(Boolean)
  if (!commits.length) throw new Error(`No changes since v${current}.`)
  const date = new Intl.DateTimeFormat('en-CA', { timeZone: 'Europe/Moscow', year: 'numeric', month: '2-digit', day: '2-digit' }).format(new Date())
  const notes = `## Changes\n\n${commits.map(subject => `- ${subject}`).join('\n')}\n`
  const entry = `## [${version}] - ${date}\n\n${notes.replace(/^## Changes\n\n/, '')}`
  const existing = existsSync('CHANGELOG.md') ? read('CHANGELOG.md') : '# Changelog\n'
  if (!existing.startsWith('# Changelog\n')) throw new Error('Expected CHANGELOG.md to start with "# Changelog".')
  const notePath = `docs/releases/${tag}.md`
  if (existsSync(notePath)) throw new Error(`${notePath} already exists; refusing to replace it.`)
  const after = [
    before[0].replace(/("version"\s*:\s*")[^"]+"/, `$1${version}"`),
    before[1].replace(/(^\[package\][\s\S]*?^version\s*=\s*")[^"]+"/m, `$1${version}"`),
    before[2].replace(/(\[\[package\]\]\s*\nname = "fonicca"\s*\nversion = ")[^"]+"/, `$1${version}"`),
    before[3].replace(/("version"\s*:\s*")[^"]+"/, `$1${version}"`),
  ]
  verifyRelease(manifests(after), tag)
  console.log(`${current} → ${version}\n\n${entry}\nCommit: Release ${version}\nTag: ${tag}\n${args.includes('--push') ? 'Push main and tag to origin (CI creates a draft).' : 'Local only; no push.'}`)
  if (args.includes('--dry-run')) return

  // Use the pnpm entrypoint when called through pnpm, including on Windows.
  for (const script of ['test', 'build']) {
    const manager = process.env.npm_execpath
    execFileSync(manager ? process.execPath : 'pnpm', manager ? [manager, script] : [script], { stdio: 'inherit', shell: !manager && process.platform === 'win32' })
  }
  if (git('status', '--porcelain', '--untracked-files=all') || paths.some((path, i) => read(path) !== before[i]))
    throw new Error('Files changed during checks; nothing was released. Review changes and run again.')
  paths.forEach((path, i) => writeFileSync(path, after[i]))
  writeFileSync('CHANGELOG.md', `# Changelog\n\n${entry}\n${existing.slice('# Changelog\n'.length).trimStart()}`)
  mkdirSync('docs/releases', { recursive: true })
  writeFileSync(notePath, notes)
  git('add', '--', ...paths, 'CHANGELOG.md', notePath)
  git('commit', '-m', `Release ${version}`)
  git('tag', '-a', tag, '-m', `Fonicca ${version}`)
  console.log(`Created commit and tag ${tag}.`)
  if (args.includes('--push')) {
    try { console.log(git('push', '--atomic', 'origin', 'HEAD:refs/heads/main', `refs/tags/${tag}`)) }
    catch (error) {
      console.error(`Release is prepared locally. Retry the push with:\ngit push --atomic origin main ${tag}`)
      throw error
    }
    console.log('Pushed. Wait for CI, then review and publish the GitHub Release draft.')
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { release(process.argv.slice(2)) }
  catch (error) { console.error(error.message); process.exitCode = 1 }
}
