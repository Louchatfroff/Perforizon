# Perforizon's mark and icons

**The mark.** A forward-leaning P with a pointed tail, wrapped by a thin orbit ring and two four-point
sparkles, in grayscale: near-black (`#111111`) with a mid-grey orbit on light backgrounds, off-white
(`#f5f5f5`) with a light-grey orbit on dark ones. The app icon is the dark-background mark on a near-black
rounded tile; the 16 and 32 px sizes drop the sparkles. The README does not use the mark.

## Masters (edit these)

| File | What |
|------|------|
| `icon.svg` | the app icon on the 16 grid: the full square with 22 % corners (Windows, Linux, favicons) |
| `icon-macos.svg` | the same tile on Apple's 1024 grid (824 px tile, 185 px corners, transparent margin) |
| `avatar.svg` | full-bleed square for services that round or crop the image (GitHub, Discord) |
| `mark.svg`, `mark-dark.svg` | the mark alone, for light and for dark backgrounds |

## Generated (don't edit)

`cargo run -p xtask -- icons` (needs `rsvg-convert` from librsvg) renders the masters into the files below (the committed ones were rendered with resvg and Pillow from the same masters):

| Output | Used by |
|--------|---------|
| `packaging/icons/hicolor/<size>/apps/horizon.png` (16–512) and `scalable/apps/horizon.svg` | Linux: copy the tree into `~/.local/share/icons/hicolor` (or `/usr/share/icons/hicolor`) |
| `packaging/linux/horizon-launcher.desktop`, `horizon.desktop` (hand-written) | Linux: the launcher installs both and the hicolor icons itself, under `$XDG_DATA_HOME` (`~/.local/share`), with its own path in `Exec` (`launcher/src/desktop.rs`; `horizon-launcher desktop-entry --remove` takes them out). Wayland matches the windows' app ids `horizon-launcher` and `horizon` |
| `packaging/icons/horizon.ico` | Windows: embedded in `horizon-client.exe`, its tools and `horizon-launcher.exe` as icon resource 1 (`build.rs` of each, Windows builds only) |
| `packaging/icons/horizon.icns` | macOS: `Horizon.app/Contents/Resources/horizon.icns`, packaged with the .dmg by `packaging/macos/package.sh` |
| `crates/horizon-theme/icons/horizon-64.png` | the game window's icon at run time (X11 and Windows title bars, `window::icon`) |
| `web/favicon.svg`, `web/favicon-32.png`, `web/apple-touch-icon.png` | the website (its `npm run assets` copies them into its `public/`) |
| `icon-1024.png`, `icon-macos-1024.png` | previews, store listings |
| `social/github-avatar-500.png` | GitHub: the organisation or account avatar |
