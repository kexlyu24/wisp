# CONTEXT.md

The big picture of Wisp: what it is, how it's put together, and why some things are the way they are. For agent rules see [AGENTS.md](AGENTS.md); for the full technical reference see [CLAUDE.md](CLAUDE.md) and [docs/BEHAVIOR.md](docs/BEHAVIOR.md).

## What Wisp is

An original, animated "dynamic notch" for Windows 10 and 11, inspired by Apple's Dynamic Island. It sits at the top centre of the primary display.

- **Closed**, it is a small black pill with the clock. It widens for what's playing: artwork, a waveform and an optional line of lyrics. It also shows a Pomodoro countdown, and briefly springs open into a pill for each new Windows notification.
- **Open** (on hover), it is a panel with pages:

| Page | What it does |
|---|---|
| Now Playing | Track, artwork, seek bar, controls, the player's own volume, synced lyrics |
| Calendar | Month view plus your own events (add, edit, delete) |
| To-do | Checklist |
| Pomodoro | Focus and break timer with adjustable lengths |
| Shelf | Drop files (or browser images) on it, drag them back out, group them into folders |
| Quick Access | Search and launch apps or your own shortcuts |
| Notifications | Recent Windows notifications |
| Settings | Switches, the background theme, updates, Quit |

- **Minimal mode:** when the closed notch would cover real content, such as browser tabs, it shrinks to a sliver.
- **Supported players:** Spotify, Spotifast, Apple Music (detection only for volume), the SoundCloud app, and Chrome or Edge (YouTube and so on).

Current version: **0.2.2**. Releases: <https://github.com/kexlyu24/wisp/releases>.

## People

- **kexlyu24**: owner and maintainer. Merges into `main` and publishes releases.
- **overhaulpxl, aoisaki01, levialavender**: collaborators with write access. They work in branches with PRs into `dev`.

## How it's built

A single native Rust executable with no UI framework. Everything is drawn into a premultiplied BGRA bitmap and shown with `UpdateLayeredWindow`.

| File | Role |
|---|---|
| `src/main.rs` | Window, the big `State`, the frame loop, `paint`, every page, input handling |
| `src/render.rs` | Thin GDI+ wrapper (loaded at runtime): shapes, icons, artwork |
| `src/media.rs` | Windows media sessions: which player to follow, track info, transport controls |
| `src/lyrics.rs` | LRCLIB with a NetEase fallback, LRC parsing, disk cache |
| `src/volume.rs` | The player's own volume: Spotifast via its CLI, others via their Volume Mixer session |
| `src/notify.rs` | Reads new toasts from the Action Center database |
| `src/tasks.rs` | To-do, Pomodoro, Calendar and Quick Access state (Win32-free, unit tested) |
| `src/todo_input.rs` | Shared single-line text editing (caret, selection, clipboard) |
| `src/settings.rs` | `settings.txt` and Start with Windows |
| `src/tray.rs` | Tray icon and menu |
| `src/updater.rs` | In-app updates from GitHub Releases (HTTPS + SHA-256 verified) |
| `src/web_drop.rs` | Downloading and validating images dropped from a browser |
| `src/diagnostics.rs` | Local, privacy-safe log (`wisp.log`) |

- **Animation** runs on springs. A worker thread ticks the UI once per compositor frame while something moves, and about 10 times a second at rest. Expensive drawing (text, icons, artwork) is cached.
- **User data** lives in `%APPDATA%\Wisp\`: settings, to-dos, calendar, shelf, Quick Access shortcuts, the custom background, cached lyrics and web images, and logs.

## Decisions worth knowing (and why)

- **Spotify volume uses its Volume Mixer session, not Spotify's in-app slider.** Measured on a real Spotify, driving its slider through UI Automation blocked for ~2 s, pulled Spotify to the front, moved in 10% key steps, and was ignored while minimized. The old workaround also left Spotify "always on top". As a result, the notch changes Spotify's level, but Spotify's own slider doesn't move.
- **Spotifast volume uses `spotifast volume` / `mute`.** These commands work even while it is minimized, and its UI doesn't. They run one at a time, and Wisp never touches Spotifast's internal files.
- **Lyrics fall back to NetEase** only when LRCLIB has no synced lyrics, and only on an exact title, artist and length match. NetEase requests are sent without cookies, because its cookie makes search return unrelated songs.
- **The default look is pure black (OLED).** Frost, Aura and Slate are real frosted glass: Wisp captures the screen behind the panel (its own layered window is never in the capture), blurs it, and tints it. The closed notch is always black.
- **Updates use HTTPS + SHA-256 verification.** Wisp pins downloads to official GitHub Releases URLs and verifies manifest version, size, SHA-256, and x64 PE headers before staging and installing (replacing the earlier Ed25519 signing flow; see `docs/UPDATES.md`).
- **Only events you typed** appear in the calendar. Wisp never imports or invents agenda items.
- **The "Music Player" picker** that used to be on the Settings page was removed in an earlier PR. Following a player is automatic, or set by `Settings::player`.

## Development flow

```text
feat/… or fix/…  ──PR──▶  dev  ──PR at release──▶  main  ──tag vX.Y.Z──▶  draft Release (CI)
```

- **CI** (`.github/workflows/rust.yml`) runs the tests, the release build and the smoke test on Windows for every push and PR to `main` and `dev`.
- **Releases** (`.github/workflows/release.yml`) build a draft from a version tag with the zip, bare exe, and (for stable tags) `wisp-update.json`. The owner reviews the draft, adds release notes, and publishes.
- **`UNRELEASED.md`** is the owner's git-ignored notes on changes since the last release. `CHANGELOG.md` is the public history.

## Open questions

- **Apple Music volume:** the adapter only detects the app; volume control isn't implemented.
- **Quick Access** uses coloured letter tiles and a blue "+ Add" button, which bends the colour rules. Keep it, or tone it down?
- **Chrome and Edge volume** applies to the whole browser, not one tab, because it is per process in the Volume Mixer.
