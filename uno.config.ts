import { defineConfig, presetIcons, presetWind4 } from 'unocss'
import presetAnimations from 'unocss-preset-animations'

// Color names follow shadcn-vue semantics so registry components work as-is.
// The values live as CSS variables in src/styles.css.
function color(name: string) {
  return { DEFAULT: `var(--${name})`, foreground: `var(--${name}-foreground)` }
}

export default defineConfig({
  // Also scan .ts: shadcn-vue keeps cva variants in components/ui/*/index.ts.
  content: {
    pipeline: { include: [/\.(vue|[jt]sx?|html)($|\?)/] },
    // Scan all sources at dev-server start too, so the first CSS response is
    // complete. Otherwise classes of components transformed after it arrive only
    // over HMR, and a window that misses that update stays half-styled.
    filesystem: ['src/**/*.{vue,ts}', 'index.html'],
  },
  // Tailwind v4 syntax that shadcn-vue components use but UnoCSS does not parse:
  // `origin-(--x)` → `origin-[var(--x)]`, `*:[svg]:x` → `[&>svg]:x`.
  preprocess: [
    m => m.replace(/-\((--[\w-]+)\)/g, '-[var($1)]'),
    m => m.replace(/^\*:\[(\w+)\]:last:/, '[&>$1:last-child]:').replace(/^\*:\[(\w+)\]:/, '[&>$1]:'),
    // `has-[>[x]]:[&>input]:y` → `[&:has(>[x])>input]:y`
    m => m.replace(/^has-\[(>\[[^\]]+\])\]:\[&([^\]]+)\]:/, '[&:has($1)$2]:'),
    // `data-[spacing=0]` → `[data-spacing="0"]`: an unquoted value starting with a digit is invalid CSS.
    m => m.replace(/data-\[([\w-]+)=(\d[\w.-]*)\]/g, 'data-[$1="$2"]'),
  ],
  presets: [
    presetWind4({ dark: 'media' }),
    presetAnimations(),
    presetIcons({ scale: 1.1, extraProperties: { 'display': 'inline-block', 'vertical-align': 'middle' } }),
  ],
  theme: {
    colors: {
      background: 'var(--background)',
      foreground: 'var(--foreground)',
      card: color('card'),
      popover: color('popover'),
      primary: { ...color('primary'), soft: 'var(--primary-soft)' },
      secondary: color('secondary'),
      muted: color('muted'),
      accent: color('accent'),
      destructive: { DEFAULT: 'var(--destructive)', foreground: '#fff' },
      border: 'var(--border)',
      input: 'var(--input)',
      ring: 'var(--ring)',
    },
    aria: {
      invalid: 'invalid="true"',
    },
    radius: {
      xs: '0.125rem',
      sm: 'calc(var(--radius) - 4px)',
      md: 'calc(var(--radius) - 2px)',
      lg: 'var(--radius)',
      xl: 'calc(var(--radius) + 4px)',
    },
  },
})
