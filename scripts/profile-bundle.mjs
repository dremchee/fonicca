// Opt-in production bundle inventory. Uses the normal Vite config without
// writing dist or changing build limits. Sizes are artifacts, not startup time.
import { build } from 'vite'
import { gzipSync } from 'node:zlib'
import { createHash } from 'node:crypto'
import { readFileSync, writeFileSync } from 'node:fs'
import { relative } from 'node:path'
import { execFileSync } from 'node:child_process'

const outputPath = process.argv[2]
if (!outputPath || process.argv.length !== 3) throw new Error('Usage: node scripts/profile-bundle.mjs <report.json>')
const started = performance.now()
const output = await build({ build: { write: false }, logLevel: 'warn' })
if (Array.isArray(output) || !('output' in output)) throw new Error('Expected one client build output')
const chunks = output.output.filter(asset => asset.type === 'chunk')
const bytes = code => Buffer.byteLength(code)
const records = chunks.map(chunk => ({
  file: chunk.fileName, bytes: bytes(chunk.code), gzipBytes: gzipSync(chunk.code).byteLength,
  sha256: createHash('sha256').update(chunk.code).digest('hex'),
  entry: chunk.isEntry, dynamic: chunk.isDynamicEntry,
  facade: chunk.facadeModuleId && relative(process.cwd(), chunk.facadeModuleId),
  imports: chunk.imports, dynamicImports: chunk.dynamicImports,
  modules: Object.entries(chunk.modules).map(([path, module]) => ({ path: relative(process.cwd(), path), renderedLength: module.renderedLength })),
}))
function scenario(root) {
  const visited = new Set()
  function include(file) {
    if (visited.has(file)) return
    const chunk = records.find(chunk => chunk.file === file)
    if (!chunk) throw new Error(`Missing static chunk ${file}`)
    visited.add(file)
    chunk.imports.forEach(include)
  }
  // main dynamically chooses a root, but that root is still startup work for
  // its window. Do not report the tiny bootstrap alone as an improvement.
  records.filter(chunk => chunk.entry || chunk.facade === root).forEach(chunk => include(chunk.file))
  const selected = records.filter(chunk => visited.has(chunk.file))
  return { chunks: [...visited], bytes: selected.reduce((sum, chunk) => sum + chunk.bytes, 0), gzipBytes: selected.reduce((sum, chunk) => sum + chunk.gzipBytes, 0) }
}
const report = {
  schema: 1, measuredAt: new Date().toISOString(),
  baseRevision: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
  sourceHashes: Object.fromEntries(['src/main.ts', 'src/App.vue', 'src/EditorWindow.vue', 'src/components/SettingsHost.vue', 'src/components/SettingsDialog.vue', 'vite.config.ts', 'pnpm-lock.yaml'].map(path => [path, createHash('sha256').update(readFileSync(path)).digest('hex')])),
  buildMs: performance.now() - started,
  js: { bytes: records.reduce((sum, chunk) => sum + chunk.bytes, 0), gzipBytes: records.reduce((sum, chunk) => sum + chunk.gzipBytes, 0) },
  libraryStartup: scenario('src/App.vue'), editorStartup: scenario('src/EditorWindow.vue'),
  chunks: records,
  assets: output.output.filter(asset => asset.type === 'asset').map(asset => ({ file: asset.fileName, bytes: typeof asset.source === 'string' ? bytes(asset.source) : asset.source.byteLength })),
  limits: 'Warm installed dependencies; Vite API build only, without type checking, native packaging, file writes or browser startup. gzip sizes are a comparison metric; local Tauri assets need not use HTTP compression. Module renderedLength is attribution before final chunk minification, not final byte contribution. Dynamic roots selected by main count toward their window startup. Other dynamic imports are excluded until requested.',
}
writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`)
console.log(JSON.stringify({ buildMs: report.buildMs, js: report.js, libraryStartup: report.libraryStartup, editorStartup: report.editorStartup }))
