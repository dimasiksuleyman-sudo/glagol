# Glagol app icon

`app-icon.png` is the original image_gen concept #3 selected by the user:
a white microphone on a red-orange rounded square. Keep this source so later
exports preserve the selected design. The exact generation prompt is below.

The committed desktop exports in `src-tauri/icons/` were generated with the
installed Tauri CLI (2.11.1), using the source without a new generation or redraw:

```powershell
node node_modules/@tauri-apps/cli/tauri.js icon docs/branding/app-icon.png --output .scratch/icon-build
Get-ChildItem -LiteralPath .scratch/icon-build -File | ForEach-Object {
  Copy-Item -LiteralPath $_.FullName -Destination (Join-Path src-tauri/icons $_.Name)
}
Copy-Item -LiteralPath src-tauri/icons/32x32.png -Destination src-tauri/icons/tray-idle.png
Copy-Item -LiteralPath src-tauri/icons/32x32.png -Destination public/favicon.png
```

Only desktop exports are copied; mobile builds are outside the current scope.
The separate circular `tray-recording.png` remains the recording indicator.
Version 0.3.0 is the first installer using this icon.

## Original prompt (built-in image_gen)

Use case: logo-brand. Generate one original Windows desktop app icon concept for Glagol (Глагол), a Russian local voice dictation and text-to-speech app. Professional memorable simple silhouette, readable at 32px. Single centered square icon, generously sized symbol, rounded square tile, front-facing flat graphic with very restrained depth, crisp smooth edges. No text captions, no mockup, no extra objects, no watermark, no existing brand logos. This is a concept for user selection. Concept 3: An original microphone emblem built from one broad rounded capsule and a single supporting U-shaped curve, with the capsule subtly resembling an exclamation mark through negative space. Warm ivory emblem on vivid burnt-orange/coral rounded square tile. Friendly confident minimal geometric silhouette.
