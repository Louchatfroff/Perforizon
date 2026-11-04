---
title: Build from source
section: Project
summary: Requirements, cargo commands, build profiles, tests, packaging and signing a release.
---

# Build from source

## Requirements

- Rust, stable toolchain (the workspace declares `rust-version = "1.85"`), with the MSVC target on Windows.
- A GPU with Vulkan support, to run the game. Perforizon is packaged for Windows x64 only.
- Python 3 for the dev tools in `tools/`.
- For signing a release: Git Bash with `openssl`.
- The game's own data is prepared from your copy of Minecraft Java 26.3, never committed: see [Install](02-install.md).

## Build and run

```sh
cargo build --profile fast -p horizon-client      # optimized, quick to link: use it to play or benchmark
cargo build --release -p horizon-client           # release profile
```

| Profile | Settings | Use |
| --- | --- | --- |
| `dev` | line tables only | Day-to-day edits. |
| `fast` | inherits release, no LTO, 16 codegen units, incremental | Playing, benchmarks, quick iteration. |
| `release` | fat LTO, one codegen unit | Shipped builds. Worth 0 to 3% over thin LTO; build time about 8 to 12 minutes. |

`panic = "abort"` is not used: shader pack compilation relies on catching panics.

The launcher is a separate workspace: from `launcher/`, `cargo build --release --locked`. Set `PERFORIZON_REPO=owner/name` to change which repository it installs from.

## Tests and checks

```sh
cargo test -p horizon-client -p horizon-client-lib --lib --bins --tests
python3 tools/pr_ready.py --quick       # fmt, clippy -D warnings, touched crates' tests
python3 -m unittest discover -s tests   # Python tool tests
```

`tools/pr_ready.py` without `--quick` also runs the parity harnesses, which need the vanilla data prepared under `.local/`. Never commit Mojang's files: `tools/check_public_tree.py` rejects them, and `git config core.hooksPath .githooks` runs it on commit.

## Package for Windows

```powershell
tools\package_windows.ps1                 # release profile
tools\package_windows.ps1 -Profile fast   # quick build
tools\package_windows.ps1 -StaticCrt      # no VC++ redistributable needed
```

This writes `dist\Perforizon-windows-x64.zip` and `dist\SHA256SUMS`.

## Sign a release

Launchers refuse a download that no key in `release-keys.txt` signed. A key pair is made with `horizon-release-sign keygen`: the public half is listed in `release-keys.txt`, the private half stays outside the repository (base64 PKCS#8) and is never committed. To make a new pair:

```sh
horizon-release-sign keygen --public-out key.pub > key.b64
```

Add the line of `key.pub` to `release-keys.txt` and rebuild the launcher. Then, from Git Bash, giving the tag and every file the release carries:

```sh
RELEASE_SIGNING_KEY=$(cat path/to/release-signing-key.b64) \
  bash tools/ci/sign_release.sh v0.1.0 dist/Perforizon-windows-x64.zip
cargo run --profile fast -p horizon-release-sign -- verify --tag v0.1.0 dist
```

That rewrites `dist/SHA256SUMS` and adds `dist/SHA256SUMS.sig`. Attach `Perforizon-windows-x64.zip`, `SHA256SUMS` and `SHA256SUMS.sig` to a GitHub release with the same tag, for example `gh release create v0.1.0 dist/Perforizon-windows-x64.zip dist/SHA256SUMS dist/SHA256SUMS.sig`. The tag you sign must be the release's tag.

`horizon-server` self-updates still target the upstream repository and asset name, so a Perforizon build of it does not update from this fork.

## Next

- How the fork follows Horizon: [Keeping up with Horizon](11-keeping-up-with-horizon.md).
- Measure your build: [Performance](08-performance.md).
- The launcher and its build: [Launcher](10-launcher.md).
