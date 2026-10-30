import assert from 'node:assert/strict'
import { test } from 'node:test'
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { createHash } from 'node:crypto'
import { catalogPath, checkModelCatalog, renderCatalog, validateCatalog } from '../scripts/model-catalog.mjs'
import { modelCatalog, stemModels, type ModelId } from '../src/model-catalog.generated.ts'
import { modelsForMode } from '../src/stem-models.ts'

const root = resolve(import.meta.dirname, '..')
const text = readFileSync(join(root, catalogPath), 'utf8')
const catalog = JSON.parse(text)

test('committed UI/native catalogs are generated from the authoritative pack manifest', () => {
  checkModelCatalog(root)
  assert.deepEqual(Object.keys(modelCatalog), Object.keys(catalog.models))
  for (const id of Object.keys(modelCatalog) as ModelId[]) {
    const source = catalog.models[id]
    assert.equal(modelCatalog[id].bytes, source.artifacts.reduce((sum: number, artifact: { size: number }) => sum + artifact.size, 0))
    assert.equal(modelCatalog[id].name, source.name)
    assert.equal(modelCatalog[id].group, source.group)
    assert.equal(modelCatalog[id].tracks, source.tracks ?? null)
  }
  assert.deepEqual(stemModels, catalog.stemOrder)
  for (const mode of ['two', 'four'] as const) {
    assert.deepEqual(modelsForMode(mode), catalog.stemOrder.filter((id: string) => catalog.models[id].tracks === (mode === 'two' ? 2 : 4)))
  }
})

test('catalog checks catch both changed inputs and hand-edited generated files', () => {
  const fixture = mkdtempSync(join(tmpdir(), 'fonicca-catalog-'))
  try {
    const write = (path: string, contents: string) => { mkdirSync(dirname(join(fixture, path)), { recursive: true }); writeFileSync(join(fixture, path), contents) }
    write(catalogPath, text)
    for (const [path, contents] of renderCatalog(text)) write(path, contents)
    checkModelCatalog(fixture)
    // Windows checkouts must agree with Linux/macOS on both generated content
    // and the fingerprint consumed by the native build script.
    const crlf = text.replace(/\n/g, '\r\n')
    assert.deepEqual(renderCatalog(crlf), renderCatalog(text))
    write(catalogPath, crlf)
    for (const [path, contents] of renderCatalog(text)) write(path, contents.replace(/\n/g, '\r\n'))
    checkModelCatalog(fixture)
    for (const [path, contents] of renderCatalog(text)) write(path, contents)
    const changed = structuredClone(catalog)
    changed.models.clap.artifacts[0].size++
    write(catalogPath, JSON.stringify(changed))
    assert.throws(() => checkModelCatalog(fixture), /stale/)
    write(catalogPath, text)
    const [path, contents] = [...renderCatalog(text)][0]!
    write(path, `${contents}// manual edit\n`)
    assert.throws(() => checkModelCatalog(fixture), /stale/)
  } finally { rmSync(fixture, { recursive: true, force: true }) }
})

test('invalid pack paths, totals, hashes, modes and duplicate ordering are rejected', () => {
  const mutations = [
    (value: typeof catalog) => { value.models.clap.artifacts[0].name = '../weights.onnx' },
    (value: typeof catalog) => { value.models.apollo.directory = '/absolute/path' },
    (value: typeof catalog) => { value.models.clap.artifacts[0].size = -1 },
    (value: typeof catalog) => { value.models.clap.artifacts[0].sha256 = 'not-a-checksum' },
    (value: typeof catalog) => { value.models.clap.artifacts[0].url = 'http://example.com/model' },
    (value: typeof catalog) => { value.models.scnetLarge.tracks = 3 },
    (value: typeof catalog) => { value.models.basicPitch.rustPrefix = 'CLAP' },
    (value: typeof catalog) => { value.stemOrder[0] = value.stemOrder[1] },
  ]
  for (const mutate of mutations) {
    const invalid = structuredClone(catalog)
    mutate(invalid)
    assert.throws(() => validateCatalog(invalid), /Model catalog:/)
  }
})

test('downloadable native graphs in the checkout match the catalog size and checksum', () => {
  let graphs = 0
  for (const model of Object.values(catalog.models) as { artifacts: { name: string, role: string, size: number, sha256: string, url: string }[] }[]) {
    for (const artifact of model.artifacts.filter(artifact => artifact.role === 'graph')) {
      const data = readFileSync(join(root, 'model-packs/v1', artifact.name))
      assert.equal(data.length, artifact.size)
      assert.equal(createHash('sha256').update(data).digest('hex'), artifact.sha256)
      assert.ok(artifact.url.endsWith(`/model-packs/v1/${artifact.name}`))
      graphs++
    }
  }
  assert.equal(graphs, 2)
})
