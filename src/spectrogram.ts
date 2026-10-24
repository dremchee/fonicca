/** Binary spectrogram viewport returned by the Rust STFT cache. */
export interface Spectrogram {
  start: number
  end: number
  columns: number
  bins: number
  sampleRate: number
  fftSize: number
  hop: number
  values: Uint8Array
}

export function parseSpectrogram(buffer: ArrayBuffer): Spectrogram {
  if (buffer.byteLength < 32)
    throw new Error('Incomplete spectrogram header')
  const header = new DataView(buffer)
  const read = (i: number) => header.getUint32(i * 4, true)
  const [version, start, end, columns, bins, sampleRate, fftSize, hop] = Array.from({ length: 8 }, (_, i) => read(i))
  if (version !== 1 || start > end || columns > 2048 || bins !== 256 || !sampleRate || !fftSize || !hop
    || buffer.byteLength !== 32 + columns * bins)
    throw new Error('Invalid spectrogram data')
  return { start, end, columns, bins, sampleRate, fftSize, hop, values: new Uint8Array(buffer, 32) }
}

export function frequencyBounds(sampleRate: number) {
  const max = sampleRate / 2
  return { min: Math.min(20, max / 16), max }
}

/** Vertical fraction, 0 = Nyquist at the top, 1 = low frequency at the bottom. */
export function frequencyY(hz: number, sampleRate: number) {
  const { min, max } = frequencyBounds(sampleRate)
  return 1 - Math.log(Math.max(min, Math.min(max, hz)) / min) / Math.log(max / min)
}

// Dark-to-bright energy map, independent of the app's light/dark theme.
const stops = [
  [0, 8, 5, 17], [0.2, 35, 15, 75], [0.45, 111, 38, 135],
  [0.7, 228, 79, 85], [0.85, 252, 157, 57], [1, 252, 250, 190],
]
const palette = new Uint8ClampedArray(256 * 4)
for (let value = 0; value < 256; value++) {
  const t = value / 255
  const hi = stops.findIndex(s => s[0] >= t)
  const a = stops[Math.max(0, hi - 1)]
  const b = stops[hi]
  const blend = a === b ? 0 : (t - a[0]) / (b[0] - a[0])
  for (let channel = 0; channel < 3; channel++)
    palette[value * 4 + channel] = a[channel + 1] + blend * (b[channel + 1] - a[channel + 1])
  palette[value * 4 + 3] = 255
}

const images = new WeakMap<Spectrogram, HTMLCanvasElement>()
function raster(data: Spectrogram) {
  let canvas = images.get(data)
  if (canvas)
    return canvas
  canvas = document.createElement('canvas')
  canvas.width = data.columns
  canvas.height = data.bins
  const context = canvas.getContext('2d')!
  const image = context.createImageData(data.columns, data.bins)
  for (let column = 0; column < data.columns; column++) {
    for (let bin = 0; bin < data.bins; bin++) {
      const value = data.values[column * data.bins + bin]
      const pixel = ((data.bins - bin - 1) * data.columns + column) * 4
      for (let channel = 0; channel < 4; channel++)
        image.data[pixel + channel] = palette[value * 4 + channel]
    }
  }
  context.putImageData(image, 0, 0)
  images.set(data, canvas)
  return canvas
}

export function drawSpectrogram(ctx: CanvasRenderingContext2D, data: Spectrogram, area: { left: number, top: number, width: number, height: number }) {
  if (!data.columns || area.height <= 0)
    return
  ctx.save()
  ctx.imageSmoothingEnabled = false
  ctx.drawImage(raster(data), area.left, area.top, area.width, area.height)
  ctx.restore()
}

export function drawFrequencyAxis(ctx: CanvasRenderingContext2D, sampleRate: number, width: number, top: number, height: number) {
  if (height <= 0)
    return
  ctx.save()
  ctx.fillStyle = 'rgba(8, 5, 17, 0.78)'
  ctx.fillRect(width - 48, top, 48, height)
  ctx.fillStyle = '#eee8f5'
  ctx.font = '10px system-ui, sans-serif'
  ctx.textAlign = 'right'
  ctx.textBaseline = 'top'
  ctx.fillText('Hz', width - 5, top + 3)
  ctx.textBaseline = 'middle'
  let lastY = -Infinity
  for (const hz of [20000, 10000, 5000, 2000, 1000, 500, 200, 100, 50, 20]) {
    if (hz >= sampleRate / 2)
      continue
    const y = top + frequencyY(hz, sampleRate) * height
    if (y < top + 22 || y > top + height - 5 || y - lastY < 24)
      continue
    ctx.fillText(hz >= 1000 ? `${hz / 1000}k` : String(hz), width - 5, y)
    lastY = y
  }
  ctx.restore()
}
