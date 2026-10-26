import { writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

// The public source repository hosts updates by default; a separate public repository is optional.
export function updaterConfig(repository) {
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository ?? '')) throw new Error('Invalid UPDATE_REPOSITORY (expected owner/repository)')
  return {
    bundle: { createUpdaterArtifacts: true },
    plugins: { updater: { endpoints: [`https://github.com/${repository}/releases/latest/download/latest.json`] } },
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const json = `${JSON.stringify(updaterConfig(process.env.UPDATE_REPOSITORY), null, 2)}\n`
  if (process.argv[2] === '--output') {
    if (!process.argv[3]) throw new Error('Expected a path after --output')
    writeFileSync(process.argv[3], json)
  } else if (process.argv.length === 2) {
    process.stdout.write(json)
  } else {
    throw new Error('Usage: updater-config.mjs [--output <file>]')
  }
}
