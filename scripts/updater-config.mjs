// The public source repository hosts updates by default; a separate public repository is optional.
export function updaterConfig(repository) {
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository ?? '')) throw new Error('Invalid UPDATE_REPOSITORY (expected owner/repository)')
  return {
    bundle: { createUpdaterArtifacts: true },
    plugins: { updater: { endpoints: [`https://github.com/${repository}/releases/latest/download/latest.json`] } },
  }
}
if (process.argv[1]?.endsWith('/updater-config.mjs')) console.log(JSON.stringify(updaterConfig(process.env.UPDATE_REPOSITORY)))
