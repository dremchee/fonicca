// Opt-in native lifecycle observation. Production source, app data and binary stay untouched.
import { createHash, randomBytes } from 'node:crypto'
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync, openSync, closeSync, writeSync } from 'node:fs'
import { tmpdir, platform } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { execFileSync, spawn } from 'node:child_process'
import { createServer } from 'node:net'
import { auditResourceReport } from './resource-profile/report.mjs'

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const args = process.argv.slice(2), smoke = args.includes('--smoke')
const positional = args.filter(arg => arg !== '--smoke')
if (positional.length !== 1) throw Error('Usage: node scripts/profile-resources.mjs [--smoke] <new-report.json>')
if (!smoke && platform() !== 'darwin') throw Error('Memory attribution requires macOS; use --smoke elsewhere')
if (platform() === 'win32' && (process.env.GITHUB_ACTIONS !== 'true' || process.env.RUNNER_ENVIRONMENT !== 'github-hosted'))
  throw Error('Windows installer smoke requires a disposable GitHub-hosted Actions runner')
const reportPath = resolve(positional[0])
if (existsSync(reportPath)) throw Error('Refusing to overwrite an existing report')
const root = mkdtempSync(join(tmpdir(), 'fonicca-resources-'))
const frontend = join(root, 'frontend'), native = join(root, 'native')
const identifier = 'app.fonicca.resourceprobe'
const sha = path => createHash('sha256').update(readFileSync(path)).digest('hex')
const ordinaryBinary = join(repo, 'src-tauri/target/debug', platform() === 'win32' ? 'fonicca.exe' : 'fonicca')
const ordinaryRelease = join(repo, 'src-tauri/target/release', platform() === 'win32' ? 'fonicca.exe' : 'fonicca')
const ordinaryBefore = existsSync(ordinaryBinary) ? sha(ordinaryBinary) : null
const releaseBefore = existsSync(ordinaryRelease) ? sha(ordinaryRelease) : null
const sourceRevision = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repo, encoding: 'utf8' }).trim()
const sourceFiles = ['src/main.ts', 'src/App.vue', 'src/EditorWindow.vue', 'src/waveform-gl.ts', 'src/stores/editor.ts',
  'src/components/EditorView.vue', 'src/composables/editor/spectrogram.ts', 'src-tauri/src/lib.rs',
  'src-tauri/src/editor.rs', 'src-tauri/src/editor/spectrogram.rs', 'src-tauri/src/editor_commands.rs',
  'src-tauri/src/editor_commands/transfer.rs', 'scripts/profile-resources.mjs', 'scripts/resource-profile/probe.ts',
  'scripts/resource-profile/controller.py', 'scripts/resource-profile/report.mjs']
const sourceSha256 = Object.fromEntries(sourceFiles.map(file => [file, sha(join(repo, file))]))
// The fixture must never inherit release signing credentials or config overrides.
const privateEnv = Object.fromEntries(Object.entries(process.env).filter(([key]) =>
  !/^(TAURI_CONFIG$|TAURI_SIGNING_|APPLE_|WINDOWS_CERTIFICATE|WINDOWS_SIGNING)/.test(key)))
const python = process.env.FONICCA_PROFILE_PYTHON || (platform() === 'win32' ? 'python' : 'python3')
let active, installedDirectory
function run(command, parameters, options = {}) {
  return new Promise((accept, reject) => {
    const { log: logName = 'build.log', echo = false, ...spawnOptions } = options
    const child = active = spawn(command, parameters, { cwd: repo, env: privateEnv, ...spawnOptions })
    const log = openSync(join(root, logName), 'a')
    child.stdout.on('data', bytes => { writeSync(log, bytes); if (echo) process.stdout.write(bytes) })
    child.stderr.on('data', bytes => { writeSync(log, bytes); if (echo) process.stderr.write(bytes) })
    let settled = false
    const finish = error => {
      if (settled) return
      settled = true; active = null; closeSync(log)
      error ? reject(error) : accept()
    }
    child.once('error', finish)
    child.once('exit', code => finish(code === 0 ? null : Error(`${command} exited ${code}; see ${root}`)))
  })
}
function audio(path, seconds) {
  const length = seconds * 44100 * 2 * 4, header = Buffer.alloc(44), second = Buffer.alloc(44100 * 8)
  header.write('RIFF'); header.writeUInt32LE(length + 36, 4); header.write('WAVEfmt ', 8)
  header.writeUInt32LE(16, 16); header.writeUInt16LE(3, 20); header.writeUInt16LE(2, 22)
  header.writeUInt32LE(44100, 24); header.writeUInt32LE(44100 * 8, 28); header.writeUInt16LE(8, 32)
  header.writeUInt16LE(32, 34); header.write('data', 36); header.writeUInt32LE(length, 40)
  for (let i = 0; i < 44100; i++) for (let lane = 0; lane < 2; lane++)
    second.writeFloatLE(.2 * Math.sin(i * Math.PI * 2 * (lane ? 330 : 220) / 44100), (i * 2 + lane) * 4)
  const fd = openSync(path, 'w')
  try { writeSync(fd, header); for (let i = 0; i < seconds; i++) writeSync(fd, second) } finally { closeSync(fd) }
}
const port = await new Promise(accept => { const server = createServer(); server.listen(0, '127.0.0.1', () => { const port = server.address().port; server.close(() => accept(port)) }) })
const config = { identifier, token: randomBytes(24).toString('hex'), rows: smoke ? 1000 : 100000,
  seconds: smoke ? 30 : 1200, cycles: smoke ? 1 : 3, smoke, port, sourceRevision,
  data: join(root, 'data'), audio: join(root, 'audio'), controllerUrl: `http://127.0.0.1:${port}`,
  binary: join(repo, 'src-tauri/target/debug', platform() === 'win32' ? 'fonicca-resource-probe.exe' : 'fonicca-resource-probe') }
console.log(`Resource fixture: ${root}`)
let failure
const interrupt = () => { if (active) active.kill('SIGTERM') }
process.on('SIGINT', interrupt)
process.on('SIGTERM', interrupt)
try {
  mkdirSync(frontend); mkdirSync(native); mkdirSync(config.data); mkdirSync(config.audio)
  for (const entry of ['src', 'index.html', 'package.json', 'uno.config.ts', 'vite.config.ts']) cpSync(join(repo, entry), join(frontend, entry), { recursive: true })
  symlinkSync(join(repo, 'node_modules'), join(frontend, 'node_modules'), platform() === 'win32' ? 'junction' : 'dir')
  cpSync(join(repo, 'docs/licenses'), join(root, 'docs/licenses'), { recursive: true })
  cpSync(join(repo, 'model-packs'), join(root, 'model-packs'), { recursive: true })
  symlinkSync(join(frontend, 'src'), join(root, 'src'), platform() === 'win32' ? 'junction' : 'dir')
  for (const entry of ['src', 'icons', 'resources', 'capabilities', 'Cargo.toml', 'Cargo.lock', 'build.rs', 'tauri.conf.json']) cpSync(join(repo, 'src-tauri', entry), join(native, entry), { recursive: true })
  cpSync(join(repo, 'scripts/resource-profile/controller.py'), join(root, 'controller.py'))
  if (sha(join(root, 'controller.py')) !== sourceSha256['scripts/resource-profile/controller.py'])
    throw Error('Controller changed while copying')
  // Verify these are the exact sources captured before any probe-only transforms.
  for (const file of sourceFiles.filter(file => file.startsWith('src/'))) {
    if (sha(join(frontend, file)) !== sourceSha256[file]) throw Error(`Source changed while copying: ${file}`)
  }
  for (const file of sourceFiles.filter(file => file.startsWith('src-tauri/'))) {
    if (sha(join(native, file.slice('src-tauri/'.length))) !== sourceSha256[file]) throw Error(`Source changed while copying: ${file}`)
  }
  for (const [file, before, after] of [
    ['Cargo.toml', 'name = "fonicca"', 'name = "fonicca-resource-probe"'],
    ['Cargo.toml', 'name = "fonicca_lib"', 'name = "fonicca_probe_lib"'],
    ['Cargo.lock', 'name = "fonicca"', 'name = "fonicca-resource-probe"'],
    ['src/main.rs', 'fonicca_lib::run()', 'fonicca_probe_lib::run()'],
    ['src/editor_commands/transfer.rs', '.title(title).inner_size', '.incognito(true).title(title).inner_size'],
  ]) { const path = join(native, file), text = readFileSync(path, 'utf8'); if (!text.includes(before)) throw Error(`Fixture source mismatch: ${file}`); writeFileSync(path, text.replace(before, after)) }
  symlinkSync(native, join(frontend, 'src-tauri'), platform() === 'win32' ? 'junction' : 'dir')
  const capabilityPath = join(native, 'capabilities/default.json'), capability = JSON.parse(readFileSync(capabilityPath))
  capability.permissions.push('core:window:allow-set-focus'); writeFileSync(capabilityPath, JSON.stringify(capability))
  const mainPath = join(frontend, 'src/main.ts'), main = readFileSync(mainPath, 'utf8')
  const ending = "void mount().catch(err => console.error('mount window failed', err))"
  if (!main.includes(ending)) throw Error('Fixture bootstrap mismatch')
  writeFileSync(mainPath, "import { profileStart } from './resource-profile'\n" + main.replace(ending, 'void profileStart(mount)'))
  cpSync(join(repo, 'scripts/resource-profile/probe.ts'), join(frontend, 'src/resource-profile.ts'))
  if (sha(join(frontend, 'src/resource-profile.ts')) !== sourceSha256['scripts/resource-profile/probe.ts'])
    throw Error('Probe changed while copying')
  writeFileSync(join(frontend, 'src/resource-profile-config.json'), JSON.stringify(config))
  const glPath = join(frontend, 'src/waveform-gl.ts')
  let gl = readFileSync(glPath, 'utf8')
  gl = gl.replace('class GlWaveRenderer {', `const counters = ((globalThis as any).__FONICCA_RESOURCE_STATS__ ??= { created: 0, destroyed: 0, destroyCalls: 0, live: 0, lanes: 0 })\nclass GlWaveRenderer {\n  private probeDestroyed = false`)
    .replace("throw new Error('WebGL2 unavailable')", "throw new Error('WebGL2 unavailable')\n    counters.created++; counters.live++")
    .replace('      this.lanes = []', '      counters.lanes -= this.lanes.length\n      this.lanes = []')
    .replace('        this.lanes.push(lane)', '        this.lanes.push(lane); counters.lanes++')
    .replace('  destroy() {', '  destroy() {\n    counters.destroyCalls++\n    if (!this.probeDestroyed) { this.probeDestroyed = true; counters.destroyed++; counters.live--; counters.lanes -= this.lanes.length }')
  writeFileSync(glPath, gl)
  const performancePath = join(frontend, 'src/editor-performance.ts')
  let performanceSource = readFileSync(performancePath, 'utf8')
  performanceSource = performanceSource.replace('    const started = now(), version = generation',
    '    const pendingProfile = ((globalThis as any).__FONICCA_PENDING_EDITOR__ ??= {})\n    pendingProfile[name] = (pendingProfile[name] ?? 0) + 1\n    ;(globalThis as any).__FONICCA_REQUEST_STARTED__?.(name)\n    const started = now(), version = generation')
    .replace('    finally { record(name, now() - started, failed, version) }',
      '    finally { pendingProfile[name]--; record(name, now() - started, failed, version) }')
  writeFileSync(performancePath, performanceSource)
  writeFileSync(join(config.data, 'settings.json'), JSON.stringify({ watchFolders: false, autoplay: false, volume: 0, language: 'en', analysisThreads: 1 }))
  audio(join(config.audio, 'short.wav'), 30); audio(join(config.audio, 'long.wav'), config.seconds)
  console.log('Building isolated production frontend…')
  await run(process.execPath, [join(repo, 'node_modules/vite/bin/vite.js'), 'build'], { cwd: frontend, echo: true })
  const tauri = JSON.parse(readFileSync(join(native, 'tauri.conf.json')))
  tauri.productName = 'Fonicca Resource Probe'; tauri.identifier = identifier; tauri.mainBinaryName = 'fonicca-resource-probe'
  tauri.build.devUrl = `http://127.0.0.1:${port}`; tauri.build.frontendDist = '../frontend/dist'
  tauri.build.beforeBuildCommand = ''; tauri.build.beforeDevCommand = ''
  tauri.app.appDirectoriesOverride = { data: config.data, config: config.data, localData: join(root, 'webview'), cache: join(root, 'cache'), log: join(root, 'logs') }
  tauri.app.windows[0] = { ...tauri.app.windows[0], width: 1280, height: 760, incognito: true }
  tauri.plugins.updater.endpoints = []
  tauri.bundle.createUpdaterArtifacts = false
  tauri.bundle.macOS = { ...tauri.bundle.macOS, signingIdentity: '-', providerShortName: null }
  tauri.bundle.windows = { ...tauri.bundle.windows, certificateThumbprint: null, signCommand: null }
  writeFileSync(join(native, 'tauri.conf.json'), JSON.stringify(tauri))
  console.log('Building isolated release bundle (distinct package/library/product, no release credentials)…')
  const bundle = platform() === 'darwin' ? 'app' : platform() === 'win32' ? 'nsis' : 'appimage'
  await run(process.execPath, [join(repo, 'node_modules/@tauri-apps/cli/tauri.js'), 'build', '--ci', '--bundles', bundle, '--', '--locked'], {
    cwd: native, echo: true, env: { ...privateEnv, CARGO_TARGET_DIR: join(repo, 'src-tauri/target'), APPIMAGE_EXTRACT_AND_RUN: '1' },
  })
  const bundleDir = join(repo, 'src-tauri/target/release/bundle')
  if (platform() === 'darwin') {
    const appBundle = join(bundleDir, 'macos/Fonicca Resource Probe.app')
    config.binary = join(appBundle, 'Contents/MacOS/fonicca-resource-probe')
    await run('codesign', ['--verify', '--deep', '--strict', appBundle])
  } else if (platform() === 'win32') {
    const { readdirSync } = await import('node:fs')
    const setup = readdirSync(join(bundleDir, 'nsis')).find(file => file.includes('Resource') && file.endsWith('.exe'))
    if (!setup) throw Error('Missing private NSIS installer')
    const destination = installedDirectory = join(root, 'installed')
    await run(join(bundleDir, 'nsis', setup), ['/S', `/D=${destination}`])
    config.binary = join(destination, 'fonicca-resource-probe.exe')
  } else {
    const { readdirSync } = await import('node:fs')
    const image = readdirSync(join(bundleDir, 'appimage')).find(file => file.includes('Resource') && file.endsWith('.AppImage'))
    if (!image) throw Error('Missing private AppImage')
    await run(join(bundleDir, 'appimage', image), ['--appimage-extract'], { cwd: root })
    config.binary = join(root, 'squashfs-root/usr/bin/fonicca-resource-probe')
    config.launch = join(root, 'squashfs-root/AppRun')
    // Inspect the actual extracted package, not a raw-binary substitute.
    await run(python, ['-c', `
from pathlib import Path
import subprocess, sys
app = Path(sys.argv[1])
print('AppImage launcher:', subprocess.check_output(['file', str(app / 'AppRun')], text=True).strip(), flush=True)
strings = subprocess.check_output(['strings', str(app / 'AppRun')], text=True)
print('AppRun environment-related strings:', flush=True)
for line in [line for line in strings.splitlines() if any(key in line for key in ['GST_', 'LD_LIBRARY_PATH', 'WEBKIT', 'APPDIR', 'XDG_DATA'])][:60]:
    print(line, flush=True)
plugins = sorted(path for path in app.rglob('*') if any(key in path.name.lower() for key in ['gstreamer', 'gstapp', 'gst-plugin-scanner']))
print('Bundled GStreamer paths (%d total, first 80):' % len(plugins), flush=True)
for path in plugins[:80]:
    print(str(path.relative_to(app)), flush=True)
`, join(root, 'squashfs-root')], { echo: true })
  }
  config.bundle = bundle
  writeFileSync(join(root, 'controller.json'), JSON.stringify(config))
  console.log('Observing private native/WebView lifecycle…')
  await run(python, [join(root, 'controller.py'), root], { echo: true, log: 'controller.log' })
} catch (error) { failure = error; console.error(String(error)) }
finally {
  if (active) active.kill()
  if (installedDirectory && existsSync(installedDirectory)) {
    const { readdirSync } = await import('node:fs')
    const uninstaller = readdirSync(installedDirectory).find(file => /^uninstall.*\.exe$/i.test(file))
    try {
      if (!uninstaller) throw Error('Private installation has no uninstaller')
      // _?= waits in place instead of spawning a temporary uninstaller child.
      await run(join(installedDirectory, uninstaller), ['/S', `_?=${installedDirectory}`])
    } catch (error) { failure ??= error }
  }
  const resultPath = join(root, 'result.json')
  const report = existsSync(resultPath) ? JSON.parse(readFileSync(resultPath)) : {
    schema: 1, status: 'failed', error: String(failure ?? 'Controller produced no report'),
    mode: smoke ? 'smoke' : 'memory', platform: platform(), checkpoints: [],
  }
  const ordinaryAfter = existsSync(ordinaryBinary) ? sha(ordinaryBinary) : null
  const releaseAfter = existsSync(ordinaryRelease) ? sha(ordinaryRelease) : null
  report.normalBinary = { before: ordinaryBefore, after: ordinaryAfter, releaseBefore, releaseAfter, unchanged: ordinaryBefore === ordinaryAfter && releaseBefore === releaseAfter }
  report.sourceSha256 = sourceSha256
  report.sourceSnapshot = 'Captured before copying and probe transforms; copied production files verified against these hashes.'
  report.sourceChangedDuringRun = sourceFiles.filter(file => !existsSync(join(repo, file)) || sha(join(repo, file)) !== sourceSha256[file])
  report.audit = auditResourceReport(report)
  if (!report.audit.passed) failure ??= Error('Resource lifecycle audit failed; inspect report.audit.checks')
  if (failure) report.runnerError = String(failure)
  mkdirSync(dirname(reportPath), { recursive: true })
  writeFileSync(reportPath, JSON.stringify(report, null, 2) + '\n')
  // CI artifact collection also receives failures that happened before controller startup.
  if (failure) for (const name of ['build.log', 'controller.log', 'native.log']) {
    const path = join(root, name)
    if (existsSync(path)) cpSync(path, `${reportPath}.${name}`)
  }
  console.log(`Report: ${reportPath}`)
  // Failed fixtures retain their logs for diagnosis; successful private audio/data are removed.
  if (!failure) rmSync(root, { recursive: true, force: true })
}
process.off('SIGINT', interrupt)
process.off('SIGTERM', interrupt)
if (failure) process.exitCode = 1
