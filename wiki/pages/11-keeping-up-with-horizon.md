---
title: Keeping up with Horizon
section: Project
summary: How Perforizon follows upstream Horizon: the hourly check, the safety checks and the manual procedure.
---

# Keeping up with Horizon

Perforizon is a fork, so it only stays useful if it keeps receiving Horizon's fixes and features. The fork repository is <https://github.com/Louchatfroff/Perforizon>; the upstream is <https://github.com/horizon-rust/horizon>, added as a git remote named `upstream`.

The rule that decides everything: world generation must stay identical to Horizon's, and the optimizations must keep working on top of whatever Horizon changes.

## The automatic sync

A scheduled workflow does this:

1. Every hour it checks whether `upstream/main` has new commits.
2. If so, it merges them into Perforizon's `main` on a work branch.
3. It runs the checks: formatting, clippy, the tests, a build, and the world-generation identity check.
4. If every check is green, the merge goes into `main` on its own.
5. If something fails, it does not touch `main`: it opens an issue or a pull request describing what broke, for a person to fix.

Releases are never automatic. A new release (tag, signed `SHA256SUMS`, upload) is a manual step by the maintainer. See [Build from source](12-build-from-source.md) for the signing procedure.

The workflow itself lives under `.github/workflows` in the repository.

## Manual procedure

Use it when the automatic sync reports a conflict, or when you maintain your own fork.

```sh
git remote add upstream https://github.com/horizon-rust/horizon.git   # once
git fetch upstream
git checkout -b sync/upstream main
git merge upstream/main
```

Resolve conflicts, keeping Perforizon's optimizations. Typical conflict areas are the terrain mesher, the renderer and the graphics options, since those are where Perforizon changes the most. Then run the same checks as the automatic job:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
python3 tools/pr_ready.py --quick
cargo build --profile fast -p horizon-client
```

The world-generation identity check is the one that must never be skipped: the same seed has to give the same world as Horizon. If the Perforizon changes ever touched worldgen, that is a bug.

Finally, push the branch and open a pull request into `main`. When it is green and merged, run the [benchmarks](08-performance.md) again if the render path changed, and update the patch notes.

## What this means for you

- You get Horizon's fixes without waiting for a manual merge, but a release only exists when the maintainer publishes it.
- Check the Versions page of the [launcher](10-launcher.md), or the releases page, for new versions.
- If something broke after an upstream merge, report it: see [Troubleshooting](13-troubleshooting.md).
