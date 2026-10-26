/** Browser-only updater transport; it never downloads or installs real software. */
export function updateCommand(cmd: string, args: any): unknown {
  const preview = location.hash === '#updates'
  if (cmd === 'update_status') return { configured: preview, development: false, supported: true, editorsOpen: false, busy: false }
  if (!preview) return undefined
  if (cmd === 'plugin:updater|check') return {
    rid: 800, currentVersion: '0.1.0', version: '0.2.0', body: 'Improved playback and waveform preview.', rawJson: {},
  }
  if (cmd === 'plugin:updater|download') return new Promise(resolve => {
    let chunks = 0
    args.onEvent.onmessage({ event: 'Started', data: { contentLength: 10_000_000 } })
    const timer = setInterval(() => {
      args.onEvent.onmessage({ event: 'Progress', data: { chunkLength: 1_000_000 } })
      if (++chunks === 10) {
        clearInterval(timer)
        args.onEvent.onmessage({ event: 'Finished' })
        resolve(801)
      }
    }, 200)
  })
  if (['plugin:updater|install', 'plugin:process|restart', 'plugin:resources|close'].includes(cmd)) return null
  return undefined
}
