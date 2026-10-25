export default {
  title: 'Audio to MIDI', action: 'Export MIDI…', description: 'Recognize notes with Spotify Basic Pitch and save a MIDI file for your DAW. Audio stays on this computer.',
  saveTitle: 'Save MIDI', sources: { file: 'Entire audio file', selection: 'Selected fragment of the edited audio', editor: 'Entire edited audio, including unsaved changes' },
  ready: 'Basic Pitch installed · local processing', download: 'The first export downloads Basic Pitch ({size}).',
  hint: 'Best for an isolated melodic instrument, such as bass, piano or guitar. Exports note timing and velocity; pitch bends are not included.',
  limits: 'Mono or stereo, up to 10 minutes. Your audio stays unchanged.',
  start: 'Export MIDI…', downloadStart: 'Download model and export…', close: 'Close', stop: 'Stop', stopping: 'Stopping after the current step…',
  reveal: 'Show in Finder', result: 'MIDI saved · {n} notes',
  stages: { preparing: 'Preparing audio…', downloading: 'Downloading Basic Pitch…', transcribing: 'Recognizing notes…', notes: 'Building notes…', saving: 'Saving MIDI…', done: 'MIDI saved', cancelled: 'Stopped', failed: 'MIDI export failed' },
}
