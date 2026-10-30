import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createHash } from 'node:crypto'
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { spawnSync } from 'node:child_process'
import { catalogPath, renderCatalog } from '../scripts/model-catalog.mjs'
import { graphArtifacts, verifyLocalGraph, verifyModelPacks, verifyRemoteGraph } from '../scripts/verify-model-packs.mjs'

const root = resolve(import.meta.dirname, '..')
const catalog = JSON.parse(readFileSync(join(root, catalogPath), 'utf8'))
const bytes = new TextEncoder().encode('tiny pinned graph\n')
const sha256 = (data: Uint8Array) => createHash('sha256').update(data).digest('hex')
const graph = { ...graphArtifacts(catalog)[0], size: bytes.length, sha256: sha256(bytes) }

function fixture() {
  const dir = mkdtempSync(join(tmpdir(), 'fonicca-model-check-'))
  const value = structuredClone(catalog)
  for (const model of Object.values(value.models) as any[]) {
    for (const artifact of model.artifacts.filter((a: any) => a.role === 'graph')) {
      artifact.size = bytes.length
      artifact.sha256 = sha256(bytes)
    }
  }
  const write = (path: string, content: string | Uint8Array) => {
    mkdirSync(dirname(join(dir, path)), { recursive: true })
    writeFileSync(join(dir, path), content)
  }
  const text = JSON.stringify(value, null, 2)
  write(catalogPath, text)
  for (const [path, content] of renderCatalog(text)) write(path, content)
  for (const graph of graphArtifacts(value)) write(graph.localPath, bytes)
  return { dir, value, write, dispose: () => rmSync(dir, { recursive: true, force: true }) }
}

test('public checks select every versioned graph and exclude all learned weights', () => {
  const graphs = graphArtifacts(catalog)
  assert.deepEqual(graphs.map((g: any) => g.modelId), ['bsRoformer', 'mdx23c'])
  assert.ok(graphs.every((g: any) => g.role === 'graph' && g.localPath.endsWith(g.name)))
  const changed = structuredClone(catalog)
  changed.models.bsRoformer.artifacts[0].url = graph.url.replace('/v1/', '/v2/')
  assert.equal(graphArtifacts(changed)[0].localPath, 'model-packs/v2/bs-roformer-native.onnx')
  for (const url of [graph.url.replace('/v1/', '/unversioned/'), graph.url.replace('/v1/', '/v1/../'), graph.url.replace(graph.name, 'different.onnx')]) {
    const invalid = structuredClone(catalog)
    invalid.models.bsRoformer.artifacts[0].url = url
    assert.throws(() => graphArtifacts(invalid), /versioned model-packs/)
  }
  for (const model of Object.values(changed.models) as any[])
    for (const artifact of model.artifacts) if (artifact.role === 'graph') artifact.role = 'weights'
  assert.throws(() => graphArtifacts(changed), /no downloadable graph/)
})

test('local graph verification checks actual bytes and rejects missing, truncated and corrupt files', async () => {
  const f = fixture()
  try {
    const selected = graphArtifacts(f.value)[0]
    assert.deepEqual(await verifyLocalGraph(selected, f.dir), { bytes: bytes.length, sha256: sha256(bytes) })
    f.write(selected.localPath, bytes.slice(1))
    await assert.rejects(verifyLocalGraph(selected, f.dir), /local.*expected.*file/)
    f.write(selected.localPath, new Uint8Array(bytes.length))
    await assert.rejects(verifyLocalGraph(selected, f.dir), /local.*SHA-256 mismatch/)
    rmSync(join(f.dir, selected.localPath))
    await assert.rejects(verifyLocalGraph(selected, f.dir), /local.*ENOENT/)
  } finally { f.dispose() }
})

test('remote verification hashes streamed body bytes without ambient credentials or trusting Content-Length', async () => {
  let options: any
  const result = await verifyRemoteGraph(graph, { fetchImpl: async (url: string, init: any) => {
    assert.equal(url, graph.url)
    options = init
    return new Response(new ReadableStream({ start(controller) {
      controller.enqueue(bytes.slice(0, 5))
      controller.enqueue(bytes.slice(5))
      controller.close()
    } }), { headers: { 'content-length': '1', 'content-encoding': 'gzip' } })
  } })
  assert.deepEqual(result, { bytes: bytes.length, sha256: graph.sha256, attempts: 1 })
  assert.equal(options.credentials, 'omit')
  assert.equal(options.headers, undefined)
  assert.ok(options.signal instanceof AbortSignal)
})

test('size and checksum failures are permanent and oversized streams are cancelled immediately', async () => {
  for (const body of [bytes.slice(1), new Uint8Array(bytes.length)]) {
    let calls = 0
    await assert.rejects(verifyRemoteGraph(graph, { fetchImpl: async () => { calls++; return new Response(body) }, sleep: async () => { throw new Error('must not retry corrupt artifacts') } }), /public.*(expected|SHA-256)/)
    assert.equal(calls, 1)
  }
  let cancelled = false, reads = 0
  await assert.rejects(verifyRemoteGraph(graph, { fetchImpl: async () => new Response(new ReadableStream({
    pull(controller) { reads++; controller.enqueue(new Uint8Array(bytes.length + 1)) },
    cancel() { cancelled = true },
  })) }), /received more than that/)
  assert.ok(cancelled)
  assert.ok(reads <= 2, 'the oversized body must not be drained')
})

test('not-found and HTTPS downgrade failures do not retry; transient HTTP and network failures retry within bounds', async () => {
  let calls = 0
  await assert.rejects(verifyRemoteGraph(graph, { fetchImpl: async () => { calls++; return new Response('missing', { status: 404 }) } }), /attempt 1\/3.*HTTP 404/)
  assert.equal(calls, 1)
  const downgraded = new Response(bytes)
  Object.defineProperty(downgraded, 'url', { value: 'http://example.test/graph' })
  await assert.rejects(verifyRemoteGraph(graph, { fetchImpl: async () => downgraded }), /away from HTTPS/)
  for (const failure of [429, 503, 'network', 'body']) {
    calls = 0
    const sleeps: number[] = []
    const result = await verifyRemoteGraph(graph, { fetchImpl: async () => {
      calls++
      if (calls === 3) return new Response(bytes)
      if (failure === 'network') throw new Error('connection reset')
      if (failure === 'body') return new Response(new ReadableStream({ start(controller) { controller.error(new Error('read reset')) } }))
      return new Response('retry', { status: failure as number })
    }, sleep: async (ms: number) => { sleeps.push(ms) } })
    assert.equal(result.attempts, 3)
    assert.deepEqual(sleeps, [250, 500])
  }
  calls = 0
  await assert.rejects(verifyRemoteGraph(graph, { attempts: 2, fetchImpl: async () => { calls++; throw new Error('offline') }, sleep: async () => {} }), /attempt 2\/2.*network\/read failure/)
  assert.equal(calls, 2)
})

test('one deadline bounds both a fetch that never resolves and a stalled or uncancellable body', { timeout: 2000 }, async () => {
  let signal: AbortSignal | undefined
  for (const body of ['fetch', 'reader', 'cancel']) {
    await assert.rejects(verifyRemoteGraph(graph, { timeoutMs: 20, attempts: 1, fetchImpl: async (_url: string, options: any) => {
      signal = options.signal
      if (body === 'fetch') return new Promise(() => {})
      return new Response(new ReadableStream({ cancel() { return body === 'cancel' ? new Promise(() => {}) : undefined } }))
    } }), /download exceeded 20 ms/)
    assert.equal(signal?.aborted, true)
  }
  await assert.rejects(verifyRemoteGraph(graph, { attempts: 0 }), /1–5 attempts/)
  await assert.rejects(verifyRemoteGraph(graph, { timeoutMs: Infinity }), /timeout/)
})

test('the complete gate validates the catalog first and never downloads weights or masks skipped public checks', async () => {
  const f = fixture()
  let calls: string[] = []
  const fetchImpl = async (url: string) => { calls.push(url); return new Response(bytes) }
  try {
    const offline = await verifyModelPacks({ root: f.dir, offline: true, fetchImpl })
    assert.equal(offline.ok, true)
    assert.equal(offline.mode, 'local-only')
    assert.equal(calls.length, 0)
    const publicReport = await verifyModelPacks({ root: f.dir, fetchImpl })
    assert.equal(publicReport.ok, true)
    assert.equal(calls.length, 2)
    assert.deepEqual(calls, graphArtifacts(f.value).map((g: any) => g.url))
    assert.ok(publicReport.graphs.every((g: any) => g.local.sha256 === g.remote.sha256))
    calls = []
    f.write(graphArtifacts(f.value)[0].localPath, 'broken')
    const broken = await verifyModelPacks({ root: f.dir, fetchImpl })
    assert.equal(broken.ok, false)
    assert.ok(broken.graphs.every((g: any) => !g.ok && !g.remote))
    assert.equal(calls.length, 0)
    f.write(catalogPath, JSON.stringify({ ...f.value, schema: 2 }))
    await assert.rejects(verifyModelPacks({ root: f.dir, fetchImpl }), /schema/)
    assert.equal(calls.length, 0)
  } finally { f.dispose() }
})

test('CLI local-only checks return a machine-readable report and a failing exit code for corrupt files', () => {
  const f = fixture()
  try {
    const script = join(root, 'scripts/verify-model-packs.mjs')
    const report = join(f.dir, 'result.json')
    let run = spawnSync(process.execPath, [script, '--offline', '--report', report], { cwd: f.dir, encoding: 'utf8' })
    assert.equal(run.status, 0, run.stderr)
    assert.equal(JSON.parse(readFileSync(report, 'utf8')).ok, true)
    f.write(graphArtifacts(f.value)[0].localPath, 'broken')
    run = spawnSync(process.execPath, [script, '--offline', '--report', report], { cwd: f.dir, encoding: 'utf8' })
    assert.equal(run.status, 1)
    assert.equal(JSON.parse(readFileSync(report, 'utf8')).ok, false)
    assert.match(run.stderr, /expected.*file/)
    run = spawnSync(process.execPath, [script, '--unexpected'], { cwd: f.dir, encoding: 'utf8' })
    assert.equal(run.status, 1)
    assert.match(run.stderr, /Usage:/)
  } finally { f.dispose() }
})
