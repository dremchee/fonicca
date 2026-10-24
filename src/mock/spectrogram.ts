// Deterministic fixture for the browser stand. Actual STFT is covered by Rust tests.
export function spectrogram(start: number, end: number, columns: number) {
  const rate = 44100
  const beatFrames = rate / 2
  const bins = 256
  const buffer = new ArrayBuffer(32 + columns * bins)
  new Uint32Array(buffer, 0, 8).set([1, start, end, columns, bins, rate, 4096, 1024])
  const values = new Uint8Array(buffer, 32)
  for (let column = 0; column < columns; column++) {
    const at = start + (end - start) * (column + 0.5) / columns
    const beat = (at % beatFrames) / beatFrames
    const kickHz = 50 + 100 * Math.exp(-beat * 20)
    for (let bin = 0; bin < bins; bin++) {
      const hz = 20 * (rate / 40) ** ((bin + 0.5) / bins)
      const kick = 0.9 * Math.exp(-beat * 8) * Math.exp(-((Math.log(hz / kickHz) / 0.18) ** 2))
      const hat = 0.55 * Math.exp(-Math.abs(beat - 0.5) * 65) * Math.min(1, hz / 3000)
      const harmonic = 0.18 * Math.exp(-((Math.log(hz / 440) / 0.03) ** 2))
      const energy = kick + hat + harmonic
      values[column * bins + bin] = Math.round(Math.max(0, Math.min(1, (20 * Math.log10(energy + 1e-6) + 100) / 100)) * 255)
    }
  }
  return buffer
}
