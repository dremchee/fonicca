export default {
  title: 'Stems', action: 'Separate into stems…', description: 'Separate a track locally into vocals, drums, bass and other instruments. The original stays unchanged.',
  model: 'Separation model',
  models: { scnetLarge: 'SCNet-large · Quality', spleeter: 'Spleeter · Fast' },
  modelDescriptions: { scnetLarge: 'For more detailed separation. Uses more memory and takes longer, especially on long tracks.', spleeter: 'Fast separation with lower resource use.' },
  folder: 'Output folder', choose: 'Choose…', noFolder: 'Choose a folder for the four WAV files.',
  download: 'First run downloads the selected model ({mb} MB). Audio stays on this computer.',
  ready: 'Model installed · local processing', limits: 'Mono or stereo, up to 10 minutes. Results are 44.1 kHz stereo WAV, 32-bit float.',
  start: 'Separate', downloadStart: 'Download model and separate', stop: 'Stop', stopping: 'Stopping after the current step…',
  close: 'Close', openProject: 'Open in multitrack', reveal: 'Show in Finder', edit: 'Open in editor',
  stages: { preparing: 'Preparing audio…', downloading: 'Downloading model…', separating: 'Separating stems…', importing: 'Adding to library…', done: 'Stems added to the library', cancelled: 'Stopped', failed: 'Separation failed' },
  instruments: { vocals: 'Vocals', drums: 'Drums', bass: 'Bass', other: 'Other' },
}
