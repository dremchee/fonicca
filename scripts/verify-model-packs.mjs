import { createHash } from 'node:crypto'
import { createReadStream } from 'node:fs'
import { readFile, stat, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { setTimeout as delay } from 'node:timers/promises'
import { fileURLToPath } from 'node:url'
import { catalogPath, checkModelCatalog, validateCatalog } from './model-catalog.mjs'

class VerificationError extends Error {
  constructor(message, retryable = false) { super(message); this.retryable = retryable }
}
const label = graph => `${graph.modelId}/${graph.name}`
const fail = message => { throw new VerificationError(message) }

// Only small, separately distributed graphs, never the multi-GB weight packs.
// Derive local version directories from the catalog URL instead of a second list.
export function graphArtifacts(catalog) {
  validateCatalog(catalog)
  const graphs = Object.entries(catalog.models).flatMap(([modelId, model]) => model.artifacts
    .filter(artifact => artifact.role === 'graph').map(artifact => {
      const path = new URL(artifact.url).pathname.match(/\/model-packs\/(v[1-9]\d*)\/([^/]+)$/)
      if (!path || path[2] !== artifact.name) fail(`${modelId}/${artifact.name}: expected a versioned model-packs graph URL`)
      return { modelId, ...artifact, localPath: `model-packs/${path[1]}/${artifact.name}` }
    }))
  if (!graphs.length) fail('catalog contains no downloadable graph artifacts')
  return graphs
}

function verifier(graph) {
  const hash = createHash('sha256')
  let bytes = 0
  return {
    add(chunk) {
      bytes += chunk.byteLength
      if (bytes > graph.size) fail(`expected ${graph.size} bytes; received more than that`)
      hash.update(chunk)
    },
    finish() {
      if (bytes !== graph.size) fail(`expected ${graph.size} bytes; received ${bytes}`)
      const sha256 = hash.digest('hex')
      if (sha256 !== graph.sha256) fail(`SHA-256 mismatch: expected ${graph.sha256}, received ${sha256}`)
      return { bytes, sha256 }
    },
  }
}

export async function verifyLocalGraph(graph, root = process.cwd()) {
  try {
    const path = resolve(root, graph.localPath)
    const info = await stat(path)
    if (!info.isFile() || info.size !== graph.size) fail(`expected a ${graph.size}-byte file at ${graph.localPath}; found ${info.size} bytes`)
    const checked = verifier(graph)
    for await (const chunk of createReadStream(path)) checked.add(chunk)
    return checked.finish()
  } catch (error) { throw new VerificationError(`${label(graph)} (local): ${error.message}`) }
}

async function remoteAttempt(graph, fetchImpl, timeoutMs) {
  const controller = new AbortController()
  let reader, response, timer
  const deadline = new Promise((_, reject) => {
    timer = setTimeout(() => {
      controller.abort()
      reject(new VerificationError(`download exceeded ${timeoutMs} ms`, true))
    }, timeoutMs)
  })
  const download = (async () => {
    try {
      response = await fetchImpl(graph.url, { signal: controller.signal, credentials: 'omit' })
      if (!response.ok) {
        void response.body?.cancel().catch(() => {})
        throw new VerificationError(`HTTP ${response.status}`, response.status === 408 || response.status === 429 || response.status >= 500)
      }
      if (response.url && new URL(response.url).protocol !== 'https:') fail('download redirected away from HTTPS')
      if (!response.body) fail('download has no body')
      reader = response.body.getReader()
      const checked = verifier(graph)
      for (;;) {
        const { done, value } = await reader.read()
        if (done) break
        checked.add(value)
      }
      return checked.finish()
    } catch (error) {
      if (error instanceof VerificationError) throw error
      throw new VerificationError(`network/read failure: ${error.message}`, true)
    }
  })()
  try { return await Promise.race([download, deadline]) }
  finally {
    clearTimeout(timer)
    controller.abort()
    // A broken stream's cancellation can itself hang; never wait past deadline.
    if (reader) void reader.cancel().catch(() => {})
    else void response?.body?.cancel().catch(() => {})
  }
}

export async function verifyRemoteGraph(graph, { fetchImpl = globalThis.fetch, timeoutMs = 15_000, attempts = 3, sleep = delay } = {}) {
  if (!Number.isInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 120_000 || !Number.isInteger(attempts) || attempts < 1 || attempts > 5)
    throw new Error('Expected timeout 1–120000 ms and 1–5 attempts')
  for (let attempt = 1; attempt <= attempts; attempt++) {
    try { return { ...await remoteAttempt(graph, fetchImpl, timeoutMs), attempts: attempt } }
    catch (error) {
      if (!error.retryable || attempt === attempts)
        throw new VerificationError(`${label(graph)} (public ${graph.url}, attempt ${attempt}/${attempts}): ${error.message}`)
      await sleep(Math.min(250 * 2 ** (attempt - 1), 2000))
    }
  }
}

export async function verifyModelPacks({ root = process.cwd(), offline = false, ...remoteOptions } = {}) {
  checkModelCatalog(root)
  const graphs = graphArtifacts(JSON.parse(await readFile(resolve(root, catalogPath), 'utf8')))
  const results = []
  for (const graph of graphs) {
    const result = { ...graph, ok: true }
    try { result.local = await verifyLocalGraph(graph, root) }
    catch (error) { result.ok = false; result.error = error.message }
    results.push(result)
  }
  // A bad checkout is a local failure; do not perform any network requests yet.
  if (!offline && results.every(result => result.ok)) {
    for (const result of results) {
      try { result.remote = await verifyRemoteGraph(result, remoteOptions) }
      catch (error) { result.ok = false; result.error = error.message }
    }
  } else if (!offline) {
    for (const result of results.filter(result => result.ok)) {
      result.ok = false
      result.error = `${label(result)} (public): skipped because a local graph failed verification`
    }
  }
  return { schema: 1, checkedAt: new Date().toISOString(), mode: offline ? 'local-only' : 'public', ok: results.every(result => result.ok), graphs: results }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  let reportPath, offline = false
  try {
    const args = process.argv.slice(2)
    while (args.length) {
      const arg = args.shift()
      if (arg === '--offline' && !offline) offline = true
      else if (arg === '--report' && !reportPath && args[0] && !args[0].startsWith('--')) reportPath = args.shift()
      else throw new Error('Usage: node scripts/verify-model-packs.mjs [--offline] [--report <file>]')
    }
    const report = await verifyModelPacks({ offline })
    if (reportPath) await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`)
    for (const graph of report.graphs) {
      if (graph.ok) console.log(`${label(graph)}: ${offline ? 'local' : 'local + public'} size/SHA-256 verified (${graph.size} bytes)`)
      else console.error(graph.error)
    }
    if (!report.ok) process.exitCode = 1
  } catch (error) {
    if (reportPath) await writeFile(reportPath, `${JSON.stringify({ schema: 1, ok: false, error: error.message }, null, 2)}\n`).catch(() => {})
    console.error(error.message)
    process.exitCode = 1
  }
}
