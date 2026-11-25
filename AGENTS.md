# AGENTS.md

Instructions for AI coding agents (Claude Code, Codex, OpenCode, Cursor and others) working on Wisp. Read this first, then [CONTEXT.md](CONTEXT.md) for the big picture. [CLAUDE.md](CLAUDE.md) is the detailed architecture reference: every state field, spring, hit code and module. Read the parts you touch.

## The project in one line

Wisp is a Dynamic Island–style notch for Windows 10/11, written in Rust against raw Win32/GDI/GDI+ plus the `windows` crate. It is **Windows-only**: it does not compile on Linux or macOS.

## Commands

The toolchain is `stable-x86_64-pc-windows-gnu`. If `cargo` is not on PATH, use `%USERPROFILE%\.cargo\bin\cargo.exe`.

```bash
cargo build --release                  # -> target/release/wisp.exe
cargo test                             # unit tests
cargo run --release -- --smoke-test    # drives the real window, writes PASS/FAIL to smoke-result.txt
```

Before you say a change is done, run all three and make sure they pass. `wisp.exe` is a GUI-subsystem app: when you run the smoke test from a script, wait for the process, then read `smoke-result.txt`. CI runs the same three steps on `windows-latest`, through `.github/workflows/rust.yml`.

Tests that need real apps or the network are `#[ignore]`d, for example `cargo test live_lrclib -- --ignored`. Never make a normal test depend on a running player, an audio device, the network, or the exact form of `%TEMP%`: the CI runner has none of these, and its TEMP is an 8.3 short path.

## Git workflow

- `main` holds released versions only. `dev` collects unreleased work.
- Branch off `dev` (`feat/…`, `fix/…`, `docs/…`) and open a PR **into `dev`**. `main` is the default branch, so pass `--base dev`.
- Never push straight to `main` or `dev`, and never force-push them. Never merge a PR or create a tag or release unless the owner (kexlyu24) asked for it in this conversation.
- When a branch merges `main` into `dev`, merge its PR with a merge commit, not a squash.
- End commit messages with a `Co-Authored-By:` line for the agent, if your tool adds one.

## Rules that are easy to break

- **Keep it original.** No code copied from other notch or Dynamic Island apps.
- **Fonts.** Never bundle or redistribute SF Pro (Apple's license). Inter is bundled, and its `Inter-LICENSE.txt` ships with every build.
- **Visual language** is Apple dark mode: `WHITE`/`SECONDARY`/`TERTIARY` labels, system `RED` for the calendar, `BLUE` for drop targets and focus. No coloured text, no card borders, no new accent colours.
- **Drawing.** Text goes through `cached_text`, never GDI straight into the frame DIB, because that clears alpha. Don't allocate surfaces per frame. Layout numbers shared by drawing and hit tests belong in the constants near the top of `src/main.rs`.
- **Frame pacing.** Anything new that animates on its own must keep the pace at least `PACE_AMBIENT`. Don't do slow work (disk, network, screen capture, image decoding) on the UI thread; use a worker thread, as `update_backdrop`, `lyrics`, `media`, `notify` and `volume` do.
- **Volume** only ever changes the player's own level, never the system volume. Spotify deliberately uses its Volume Mixer session; see CONTEXT.md for why.
- **The calendar** only shows events the user typed. Never invent, sample or import events.
- **Privacy.** Nothing leaves the PC except lyrics lookups (LRCLIB, then NetEase), update checks, and image downloads the user dropped. Logs never contain user text, clipboard contents, full URLs or credentials.
- **Updates are verified via HTTPS + SHA-256.** Never weaken `src/updater.rs`'s checks (official GitHub release URL pinning, manifest version/size/SHA-256, and x64 PE validation). See `docs/UPDATES.md` for the trust model and asset contract.
- **Demo and README GIFs** may only show public-domain songs ("Amazing Grace") or placeholders. No copyrighted songs, lyrics, album art or Spotify branding.

## When you change behaviour

Update the docs in the same PR:

- `docs/BEHAVIOR.md`: the detailed behaviour reference.
- `README.md`: the short GitHub overview. Keep it short.
- `CLAUDE.md`: architecture notes for agents.
- `CHANGELOG.md`: at release time.

The owner keeps a git-ignored `UNRELEASED.md`, in Indonesian, listing changes since the last release. If it exists in your checkout, add your change to it.

## Testing in the real app

Never move the user's mouse or keyboard to test. To check something on screen, prefer one of these:

- a temporary, clearly marked `// TEMP:` hook read from an environment variable, removed before committing;
- a scratch `APPDATA` folder, so the user's settings, to-dos, calendar and shelf aren't touched;
- a screen capture (for example `ffmpeg -f gdigrab`).

## Releasing (owner only)

1. Bump `Cargo.toml` and merge `dev` → `main`.
2. Tag from `main` (`git tag vX.Y.Z` and `git push origin vX.Y.Z`). `.github/workflows/release.yml` then builds, tests, packages `wisp-update.json` (for stable tags) via `scripts/package-update.py`, and drafts a Release with the zip, exe, and manifest.
3. The owner reviews the draft on GitHub, adds the release notes, and publishes (or uses `python scripts/sign-release.py vX.Y.Z` as a manual fallback to re-attach `wisp-update.json`).

Hotfix tags are `vX.Y.Zhf`, with Cargo version `X.Y.Z+hf`.
