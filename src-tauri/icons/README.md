# Application icons

`app-icon-dark.svg` and `app-icon-light.svg` embed the selected neon CD artwork.
The rounded clip removes the preview backdrop and keeps a transparent exterior.
`app-icon.svg` is the dark default used for the installed bundle.

Regenerate each variant with:

```sh
pnpm tauri icon src-tauri/icons/app-icon-dark.svg -o /tmp/fonicca-icon-dark
pnpm tauri icon src-tauri/icons/app-icon-light.svg -o /tmp/fonicca-icon-light
```

Copy the dark desktop outputs to this directory for the bundle. For each variant,
copy `icon.png` to `icon-{dark,light}.png` and `128x128.png` to
`128x128-{dark,light}.png`. The Rust application selects the native icon from the
saved appearance and system theme; the About panel follows the displayed theme.
Finder uses the bundled static icon; the running macOS Dock icon changes live.
