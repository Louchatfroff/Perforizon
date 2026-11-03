# Installing and releasing Perforizon

Perforizon's releases live at <https://github.com/Louchatfroff/Perforizon/releases>. Nothing here pushes
anything: uploading a release is a step you do yourself.

## Players: install

PowerShell, no admin rights:

```powershell
irm https://raw.githubusercontent.com/Louchatfroff/Perforizon/main/packaging/windows/install-perforizon.ps1 | iex
```

It downloads the latest release's `Perforizon-windows-x64.zip`, checks it against the release's `SHA256SUMS`
(and the `SHA256SUMS` inside the archive), unpacks to `%LOCALAPPDATA%\Perforizon` and adds a Start Menu
shortcut. This only checks integrity: the script does not verify `SHA256SUMS.sig`, so a release whose zip and sums were both replaced would pass. Only the launcher verifies the signature; to authenticate a manual install, compare the SHA-256 the script prints with the one the author publishes. Options: `-Version v0.1.0`, `-Dir D:\Perforizon`, `-Repo owner/name`. Then read `PLAY.txt` in that
folder to prepare the game files from your own Minecraft (`horizon-prepare.exe --prepare --server-jar ...`).

Or unzip the release by hand: it holds `horizon-client.exe`, `horizon-ctl.exe`, `horizon-prepare.exe`,
`PLAY.txt`, `README.txt` (Perforizon's features) and `SHA256SUMS`.

## Launcher

The existing launcher (`launcher/`, its own workspace) is adapted to the fork: it lists and installs releases of
`Louchatfroff/Perforizon`, looks for the asset `Perforizon-windows-x64.zip`, and checks the signed checksums
against the keys in `crates/horizon-release-sign/release-keys.txt` (which now include the Perforizon key).
The repository is set when the launcher is built: `PERFORIZON_REPO=owner/name cargo build -p horizon-launcher`
(default `Louchatfroff/Perforizon`). Only the Windows asset is renamed; other platforms are not packaged by this
fork. Build it from `launcher/` with `cargo build --release --locked`.

## Maintainers: build a release

```powershell
tools\package_windows.ps1                 # release profile; -Profile fast for a quick build; -StaticCrt for no VC++ redistributable
```

This writes `dist\Perforizon-windows-x64.zip` and `dist\SHA256SUMS` (the zip's hash).

## Signing a release

Launchers refuse a download that no key in `release-keys.txt` signed. The Perforizon key pair was generated with
`horizon-release-sign keygen`; the public half is listed in `release-keys.txt`, the private half is in
`%USERPROFILE%\.perforizon\release-signing-key.b64` (base64 PKCS#8, outside the repository; back it up and never
commit it). To make a new pair: `horizon-release-sign keygen --public-out key.pub > key.b64`, then add
`key.pub`'s line to `release-keys.txt` and rebuild the launcher.

Sign from Git Bash (needs `openssl`), giving the tag and every file the release carries:

```sh
RELEASE_SIGNING_KEY=$(cat ~/.perforizon/release-signing-key.b64) \
  bash tools/ci/sign_release.sh v0.1.0 dist/Perforizon-windows-x64.zip
cargo run --profile fast -p horizon-release-sign -- verify --tag v0.1.0 dist
```

That rewrites `dist/SHA256SUMS` and adds `dist/SHA256SUMS.sig`. Attach `Perforizon-windows-x64.zip`,
`SHA256SUMS` and `SHA256SUMS.sig` to a GitHub release tagged `v0.1.0` (for example `gh release create v0.1.0
dist/Perforizon-windows-x64.zip dist/SHA256SUMS dist/SHA256SUMS.sig`). The tag you sign must be the release's tag.

Note: `horizon-server` self-updates still target the upstream repository and asset name (`crates/horizon-server/src/update.rs`), so it does not update from this fork.
