<div align="center">

# Wisp

**A Dynamic Island–style notch for Windows, inspired by Apple's design.**

What's playing, synced lyrics, notifications, a calendar, to-dos, a Pomodoro timer and a file shelf, all in a small notch at the top of your screen.

![Windows 10 | 11](https://img.shields.io/badge/Windows-10%20%7C%2011-0078D4?logo=windows&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-stable-000000?logo=rust)
![License: MIT](https://img.shields.io/badge/license-MIT-blue)

<img src="assets/previews/now-playing.gif" alt="The notch turns from a clock into Now Playing, then opens on hover" width="640">

**[⬇ Download the latest release](https://github.com/kexlyu24/wisp/releases/latest)**

</div>

## Features

- 🎵 **Now Playing** for Spotify, Apple Music, Spotifast, the SoundCloud app, and Chrome or Edge, with artwork, seeking, controls and a volume slider for that app only
- 🎤 **Synced lyrics** in the panel and in the closed notch, from LRCLIB with NetEase as a fallback, cached for offline use
- 🔔 **Notifications** from any app pop up in the notch, and a Notification Center tab keeps the recent ones
- 📅 **Calendar** with your own events, ✅ **to-dos**, and a ⏱️ **Pomodoro** timer with adjustable lengths
- 📥 **File shelf**: drop files on the notch, drag them back out, and group them into folders
- 🪶 **Minimal mode**: shrinks to a sliver when it would hide something, like browser tabs
- 🚀 **Quick Access**: search and launch apps, or add your own shortcuts
- ⚙️ **Settings** right inside the notch, including the background (black by default, frosted glass, or your own image), and every movement runs on springs

<table>
  <tr>
    <td><img src="assets/previews/lyrics.gif" alt="Synced lyrics" width="100%"></td>
    <td><img src="assets/previews/notifications.gif" alt="Notifications" width="100%"></td>
  </tr>
  <tr>
    <td><img src="assets/previews/pages.gif" alt="Calendar, to-do, Pomodoro, shelf and settings pages" width="100%"></td>
    <td><img src="assets/previews/shelf.gif" alt="File shelf" width="100%"></td>
  </tr>
</table>

## Install

1. Download the zip from **[Releases](https://github.com/kexlyu24/wisp/releases/latest)** and unzip it.
2. Run `wisp.exe`. There's no installer. If Windows warns you, click **More info → Run anyway**, since the app isn't signed.
3. Hover the notch at the top of your screen. Settings and Quit are in the notch's gear tab and in the tray icon.

Wisp uses Apple's **SF Pro Display** if you've [installed it](https://developer.apple.com/fonts/), and otherwise the built-in Inter font.

**Privacy:** everything stays in `%APPDATA%\Wisp`. The only network requests are lyrics lookups (song title and artist). Notifications are read locally and never stored or sent.

## Build

Needs [Rust](https://rustup.rs) with the `stable-x86_64-pc-windows-gnu` toolchain.

```bash
cargo build --release
```

```bash
cargo test
```

Every behavior, size and timing is documented in **[docs/BEHAVIOR.md](docs/BEHAVIOR.md)**. Pull requests go into `dev`.

## Contributors

<table>
  <tr>
    <td align="center" width="230">
      <a href="https://github.com/kexlyu24"><img src="https://avatars.githubusercontent.com/u/201588681?v=4&s=200" width="88" height="88" alt="kexlyu24"></a><br>
      <a href="https://github.com/kexlyu24"><b>kexlyu24</b></a><br>
      <sub>🛠️ Creator &amp; maintainer</sub>
    </td>
    <td align="center" width="230">
      <a href="https://github.com/overhaulpxl"><img src="https://avatars.githubusercontent.com/u/179252797?v=4&s=200" width="88" height="88" alt="overhaulpxl"></a><br>
      <a href="https://github.com/overhaulpxl"><b>overhaulpxl</b></a><br>
      <sub>🔊 Volume · 📅 Calendar · ⚡ Performance</sub>
    </td>
  </tr>
</table>

Thanks for helping make Wisp better! Want to join? Open an issue, or a pull request into `dev`.

## Credits

Lyrics from [LRCLIB](https://lrclib.net) and NetEase Cloud Music. [Inter](https://rsms.me/inter/) font, SIL Open Font License 1.1. Inspired by Apple's Dynamic Island; not affiliated with Apple.

## License

[MIT](LICENSE)
