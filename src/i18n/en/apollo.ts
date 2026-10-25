export default {
  title: 'Apollo music restoration', action: 'Restore with Apollo…',
  description: 'Restore musical detail lost through lossy compression. Listen to both versions to choose the result you prefer.',
  ready: 'Apollo installed · local processing', download: 'First run downloads Apollo ({size}). Audio stays on this computer.',
  output: 'Saves a new “Apollo” WAV beside the original and adds it to the library. The original stays unchanged.',
  limits: 'Mono or stereo, up to 10 minutes. Output: 44.1 kHz WAV, 32-bit float. Processing runs in the background.',
  start: 'Restore', downloadStart: 'Download and restore', stop: 'Stop', stopping: 'Stopping after the current fragment…', close: 'Close',
  original: 'Original', result: 'After Apollo', edit: 'Open in editor', reveal: 'Show in Finder',
  stages: { preparing: 'Preparing audio…', downloading: 'Downloading Apollo…', processing: 'Restoring with Apollo…', importing: 'Adding to library…', done: 'Restored copy added to the library', cancelled: 'Stopped', failed: 'Restoration failed' },
}
