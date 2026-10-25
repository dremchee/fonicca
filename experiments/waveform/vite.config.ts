import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vite'

// Independent dependency cache and port: opening the bench does not re-optimize the app.
export default defineConfig({
  root: fileURLToPath(new URL('../../', import.meta.url)),
  cacheDir: 'node_modules/.vite-waveform-comparison',
  build: {
    outDir: 'experiments/waveform/dist', emptyOutDir: true,
    rolldownOptions: { input: fileURLToPath(new URL('../../waveform-compare.html', import.meta.url)) },
  },
  server: { host: '127.0.0.1', port: 1425, strictPort: true },
})
