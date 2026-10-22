import { defineConfig, presetIcons, presetWind4 } from 'unocss'

export default defineConfig({
  presets: [
    presetWind4(),
    presetIcons({ scale: 1.1, extraProperties: { 'display': 'inline-block', 'vertical-align': 'middle' } }),
  ],
  theme: {
    colors: {
      bg: 'var(--bg)',
      panel: 'var(--panel)',
      raised: 'var(--raised)',
      line: 'var(--line)',
      fg: 'var(--fg)',
      muted: 'var(--muted)',
      accent: 'var(--accent)',
      danger: 'var(--danger)',
    },
  },
})
