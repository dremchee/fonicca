import { readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

export const catalogPath = 'model-packs/catalog.json'
export const generatedPaths = ['src/model-catalog.generated.ts', 'src-tauri/src/stems/catalog.generated.rs']
const groups = ['analysis', 'separation', 'restoration', 'transcription']
const safeName = value => typeof value === 'string' && /^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(value) && value !== '.' && value !== '..'
const fail = message => { throw new Error(`Model catalog: ${message}`) }

export function validateCatalog(catalog) {
  if (catalog?.schema !== 1 || !catalog.models || Array.isArray(catalog.models)) fail('unsupported schema')
  const variants = new Set(), prefixes = new Set(), directories = new Set()
  for (const [id, model] of Object.entries(catalog.models)) {
    if (!/^[a-z][A-Za-z0-9]*$/.test(id)) fail(`invalid ID ${id}`)
    if (!model || typeof model.name !== 'string' || !model.name.trim() || /[\x00-\x1f\x7f]/.test(model.name)) fail(`${id}: invalid name`)
    if (!groups.includes(model.group)) fail(`${id}: unknown group`)
    if (typeof model.rustPrefix !== 'string' || !/^(?:[A-Z][A-Z0-9_]*)?$/.test(model.rustPrefix) || prefixes.has(model.rustPrefix)) fail(`${id}: invalid/duplicate Rust prefix`)
    prefixes.add(model.rustPrefix)
    if (!(model.directory === '' && id === 'clap') && !safeName(model.directory)) fail(`${id}: unsafe directory`)
    if (directories.has(model.directory)) fail(`${id}: duplicate directory`)
    directories.add(model.directory)
    if (model.group === 'separation') {
      if (![2, 4].includes(model.tracks) || typeof model.rustVariant !== 'string' || !/^[A-Z][A-Za-z0-9]*$/.test(model.rustVariant) || variants.has(model.rustVariant)) fail(`${id}: invalid stem mode/variant`)
      if (id !== model.rustVariant[0].toLowerCase() + model.rustVariant.slice(1)) fail(`${id}: ID must match Rust camelCase serialization`)
      variants.add(model.rustVariant)
    } else if (model.tracks !== undefined || model.rustVariant !== undefined) fail(`${id}: only separation models have track modes`)
    if (!Array.isArray(model.artifacts) || !model.artifacts.length) fail(`${id}: missing artifacts`)
    const names = new Set()
    let total = 0
    for (const artifact of model.artifacts) {
      if (!safeName(artifact.name) || names.has(artifact.name)) fail(`${id}: unsafe/duplicate artifact`)
      names.add(artifact.name)
      if (!Number.isSafeInteger(artifact.size) || artifact.size <= 0) fail(`${id}: invalid artifact size`)
      if (typeof artifact.sha256 !== 'string' || !/^[a-f0-9]{64}$/.test(artifact.sha256)) fail(`${id}: invalid SHA-256`)
      if (!['model', 'graph', 'weights'].includes(artifact.role)) fail(`${id}: unknown artifact role`)
      let url
      try { url = new URL(artifact.url) } catch { fail(`${id}: invalid URL`) }
      if (url.protocol !== 'https:' || url.username || url.password || url.hash || /[\x00-\x20\x7f]/.test(artifact.url)) fail(`${id}: invalid download URL`)
      total += artifact.size
    }
    if (!Number.isSafeInteger(total)) fail(`${id}: download total is too large`)
  }
  for (const id of ['clap', 'apollo', 'basicPitch']) if (!catalog.models[id]) fail(`missing ${id}`)
  if (catalog.models.clap.artifacts.length !== 1 || catalog.models.basicPitch.artifacts.length !== 1) fail('CLAP and Basic Pitch require a single model artifact')
  const ids = Object.keys(catalog.models).filter(id => catalog.models[id].group === 'separation')
  if (!Array.isArray(catalog.stemOrder) || new Set(catalog.stemOrder).size !== ids.length || catalog.stemOrder.length !== ids.length || !ids.every(id => catalog.stemOrder.includes(id))) fail('stem order must include every separation model exactly once')
  return catalog
}

export function catalogFingerprint(text) {
  let hash = 14695981039346656037n
  for (const byte of Buffer.from(text.replace(/\r\n/g, '\n'))) hash = BigInt.asUintN(64, (hash ^ BigInt(byte)) * 1099511628211n)
  return hash.toString(16).padStart(16, '0')
}

export function renderCatalog(text) {
  const { models, stemOrder } = validateCatalog(JSON.parse(text))
  const header = `// Generated from ${catalogPath}. Run pnpm models:generate; do not edit.\n// Catalog FNV-1a64: ${catalogFingerprint(text)}\n`
  const total = model => model.artifacts.reduce((sum, artifact) => sum + artifact.size, 0)
  const frontend = Object.fromEntries(Object.entries(models).map(([id, model]) => [id, {
    name: model.name, group: model.group, bytes: total(model), tracks: model.tracks ?? null,
  }]))
  const ts = `${header}export const modelCatalog = ${JSON.stringify(frontend, null, 2)} as const\n\nexport const stemModels = ${JSON.stringify(stemOrder)} as const\nexport type StemModel = typeof stemModels[number]\nexport type ModelId = keyof typeof modelCatalog\nexport type ModelGroup = typeof modelCatalog[ModelId]['group']\n`
  const rust = [header.trimEnd()]
  const q = JSON.stringify
  const stems = Object.entries(models).filter(([, model]) => model.group === 'separation')
  for (const [id, model] of Object.entries(models)) {
    const prefix = model.rustPrefix ? `${model.rustPrefix}_` : ''
    rust.push(`pub const ${prefix}TOTAL: u64 = ${total(model)};`)
    if (id === 'clap') {
      const artifact = model.artifacts[0]
      rust.push(`pub const CLAP_FILE: &str = ${q(artifact.name)};`, `pub const CLAP_URL: &str = ${q(artifact.url)};`, `pub const CLAP_SHA256: &str = ${q(artifact.sha256)};`)
      continue
    }
    if (id === 'apollo' || id === 'basicPitch') rust.push(`pub const ${prefix}DIRECTORY: &str = ${q(model.directory)};`)
    if (id === 'basicPitch' || id === 'apollo') rust.push(`pub const ${prefix}FILE: &str = ${q(model.artifacts[0].name)};`)
    rust.push(`const ${prefix}FILES: [Artifact; ${model.artifacts.length}] = [`)
    for (const artifact of model.artifacts) rust.push(`    Artifact { name: ${q(artifact.name)}, size: ${artifact.size}, hash: ${q(artifact.sha256)}, url: ${q(artifact.url)} },`)
    rust.push('];')
  }
  rust.push('impl Model {', `    pub const ALL: [Self; ${stems.length}] = [${stems.map(([, model]) => `Self::${model.rustVariant}`).join(', ')}];`, "    pub fn id(self) -> &'static str {", '        match self {')
  for (const [id, model] of stems) rust.push(`            Self::${model.rustVariant} => ${q(id)},`)
  rust.push('        }', '    }', '}')
  for (const [signature, expression] of [
    ["fn files(model: Model) -> &'static [Artifact]", model => `&${model.rustPrefix ? `${model.rustPrefix}_` : ''}FILES`],
    ["fn directory_name(model: Model) -> &'static str", model => q(model.directory)],
    ['pub(super) fn tracks(model: Model) -> u8', model => String(model.tracks)],
    ['pub fn total(model: Model) -> u64', model => `${model.rustPrefix ? `${model.rustPrefix}_` : ''}TOTAL`],
  ]) {
    rust.push(`${signature} {`, '    match model {')
    for (const [, model] of stems) rust.push(`        Model::${model.rustVariant} => ${expression(model)},`)
    rust.push('    }', '}')
  }
  return new Map([[generatedPaths[0], ts], [generatedPaths[1], `${rust.join('\n')}\n`]])
}

export function checkModelCatalog(root = process.cwd()) {
  const text = readFileSync(resolve(root, catalogPath), 'utf8')
  for (const [path, expected] of renderCatalog(text)) {
    let actual
    try { actual = readFileSync(resolve(root, path), 'utf8') } catch { /* Report a missing generated file alongside stale ones. */ }
    if (actual?.replace(/\r\n/g, '\n') !== expected) fail(`${path} is stale; run pnpm models:generate`)
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    if (process.argv.slice(2).some(argument => argument !== '--check')) fail('usage: node scripts/model-catalog.mjs [--check]')
    if (process.argv.includes('--check')) checkModelCatalog()
    else for (const [path, text] of renderCatalog(readFileSync(catalogPath, 'utf8'))) writeFileSync(path, text)
    console.log(process.argv.includes('--check') ? 'Model catalog is current' : 'Model catalog generated')
  } catch (error) { console.error(error.message); process.exitCode = 1 }
}
