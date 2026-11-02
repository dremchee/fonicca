// Hold IPC responses to exercise the real Pinia player without an audio device.
export async function checkPlayerTransitions(page) {
  await page.waitForFunction(() => document.querySelector('#app')?.__vue_app__?.config.globalProperties.$pinia?._s.get('library')?.sampleAt(1))
  const result = await page.evaluate(async () => {
    const pinia = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia
    const player = pinia._s.get('player'), library = pinia._s.get('library')
    const original = window.__TAURI_INTERNALS__.invoke
    const pending = [], controls = []
    const held = new Set()
    const settings = pinia._s.get('settings').settings
    const oldSync = { syncEnabled: settings.syncEnabled, projectBpm: settings.projectBpm, projectKey: settings.projectKey }
    const tick = () => new Promise(resolve => setTimeout(resolve, 0))
    window.__TAURI_INTERNALS__.invoke = (cmd, args, options) => {
      if (cmd === 'play') return new Promise((resolve, reject) => pending.push({ args, resolve, reject }))
      if (held.has(cmd)) return new Promise((resolve, reject) => controls.push({ cmd, resolve, reject }))
      if (cmd === 'playback_position') return Promise.resolve(.1)
      return original(cmd, args, options)
    }
    const first = library.sampleAt(0), second = library.sampleAt(1)
    const none = { stretch: 1, semitones: 0 }, adjusted = { stretch: 1.5, semitones: 2 }
    const checks = []
    try {
      await player.stop()
      const a = player.play(first), b = player.play(second)
      pending[1].resolve(adjusted); await b
      pending[0].resolve(none); await a
      checks.push({ name: 'latest start wins', passed: player.playingId === second.id && player.adjust.stretch === 1.5 })
      await player.stop(); pending.length = 0
      const stopped = player.play(first)
      await player.stop()
      pending[0].resolve(none); await stopped
      checks.push({ name: 'stop suppresses late start', passed: player.playingId === null && player.position === 0 })
      await player.stop(); pending.length = 0
      const stale = player.play(first), fresh = player.play(second)
      pending[1].resolve(adjusted); await fresh
      pending[0].reject(Error('late failed decode')); await stale
      checks.push({ name: 'stale error preserves new track', passed: player.playingId === second.id && player.error === null })
      for (const operation of ['pause', 'resume', 'seek']) {
        await player.stop(); pending.length = 0; controls.length = 0
        const initial = player.play(first); pending[0].resolve(none); await initial
        if (operation === 'resume') player.paused = true
        held.add(operation)
        const old = operation === 'pause' ? player.pause()
          : operation === 'seek' ? player.seek(first, .2) : player.toggle(first)
        const next = player.play(second); pending[1].resolve(adjusted); await next
        controls[0].reject(Error(`late ${operation} error`)); await old
        held.delete(operation)
        checks.push({ name: `stale ${operation} error preserves new track`, passed: player.playingId === second.id && player.error === null })
      }
      await player.stop(); pending.length = 0
      const initial = player.play(first); pending[0].resolve(none); await initial
      const switching = player.play(second, .25)
      Object.assign(settings, { syncEnabled: true, projectBpm: 123, projectKey: null })
      await tick()
      const retargeted = pending[2]
      checks.push({ name: 'tempo change retargets pending track', passed: retargeted?.args.id === second.id && retargeted?.args.start === .25 && retargeted?.args.sync?.bpm === 123 })
      if (retargeted) { retargeted.resolve(adjusted); await tick() }
      pending[1].resolve(none); await switching
      checks.push({ name: 'old tempo response cannot replace new adjustment', passed: player.playingId === second.id && player.adjust.stretch === 1.5 })
      await player.stop(); pending.length = 0
      const loading = player.play(second, .2)
      const seek = player.seek(second, .3)
      checks.push({ name: 'seek during preparation starts the requested point with current sync', passed: pending.length === 2 && pending[1].args.id === second.id && pending[1].args.start === .3 && pending[1].args.sync?.bpm === 123 })
      pending[0].resolve(none); await loading
      pending[1].resolve(adjusted); await seek
      await player.stop(); pending.length = 0; controls.length = 0
      const audible = player.play(first); pending[0].resolve(none); await audible
      const resync = player.play(first, .2)
      held.add('pause')
      const pause = player.pause(); controls[0].resolve(.25); await pause
      held.delete('pause')
      pending[1].resolve(adjusted); await resync
      checks.push({ name: 'pause cancels pending resync and keeps its exact source position', passed: player.playingId === first.id && player.paused && player.position === .25 })
      const resume = player.toggle(first)
      checks.push({ name: 'resume re-renders canceled sync at the paused position', passed: pending[2]?.args.id === first.id && pending[2]?.args.start === .25 && pending[2]?.args.sync?.bpm === 123 })
      pending[2].resolve(adjusted); await resume
      return checks
    } finally {
      await player.stop()
      Object.assign(settings, oldSync)
      await tick()
      window.__TAURI_INTERNALS__.invoke = original
    }
  })
  if (result.some(check => !check.passed)) throw Error(JSON.stringify(result))
  return { passed: result.map(check => check.name) }
}

export async function checkEditorPlaybackTransitions(page) {
  await page.waitForFunction(() => document.querySelector('#app')?.__vue_app__?.config.globalProperties.$pinia?._s.get('editor')?.state)
  const result = await page.evaluate(async () => {
    const editor = document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('editor')
    const original = window.__TAURI_INTERNALS__.invoke
    const pending = [], calls = []
    const tick = () => new Promise(resolve => setTimeout(resolve, 0))
    window.__TAURI_INTERNALS__.invoke = (cmd, args, options) => {
      calls.push(cmd)
      if (cmd === 'editor_play') return new Promise((resolve, reject) => pending.push({ resolve, reject }))
      if (cmd === 'playback_position') return Promise.resolve(.1)
      return original(cmd, args, options)
    }
    try {
      await editor.stopPlayback()
      const first = editor.play(); await tick()
      const before = calls.filter(cmd => cmd === 'stop').length
      await editor.stopPlayback()
      const stoppedPending = calls.filter(cmd => cmd === 'stop').length === before + 1
      pending[0].resolve(); await first
      const stayedStopped = !editor.playing && editor.playhead === null
      const old = editor.play(); await tick()
      const fresh = editor.play(); await tick()
      pending[2].resolve(); await fresh
      pending[1].resolve(); await old
      const latestWins = editor.playing
      const failed = editor.play(); await tick()
      pending[3].reject(Error('test current playback failure')); await failed
      return { stoppedPending, stayedStopped, latestWins, failureClearedPlaying: !editor.playing && editor.playhead === null }
    } finally {
      await editor.stopPlayback()
      window.__TAURI_INTERNALS__.invoke = original
    }
  })
  if (Object.values(result).some(value => !value)) throw Error(JSON.stringify(result))
  return { passed: ['pending editor play issues native stop', 'late editor play stays stopped', 'latest editor play wins', 'failed restart clears stale playing state'] }
}
