# In-app updates

Open **Settings > Updates**. Checks use the latest stable GitHub Release in
`kexlyu24/wisp`; drafts and prereleases are ignored. Automatic checks are enabled
by default and run at startup only when the last attempt was at least one day
ago. Turn them off in this page to use manual checks instead.

Nothing downloads or installs without an explicit click. Download runs on a
worker, shows progress, supports cancellation/retry and verifies an Ed25519
signature, size, SHA-256 and x64 PE header before becoming ready. Cancellation
can take up to a network timeout while a synchronous request is blocked.

**Restart & update** closes Wisp normally, waits for the previous process,
replaces only `wisp.exe`, then starts the new executable. Its startup health
marker must arrive within 15 seconds or the helper attempts to restore and
restart the old executable. This is a startup check, not continuous crash
monitoring. Keep only one Wisp instance running when updating. The executable
must be named `wisp.exe`, and its directory must be writable; no elevation is
requested. A failed operation is reported with a native message and, when
possible, `%APPDATA%\Wisp\update-error.txt`.

The helper and `wisp-backup-<pid>.exe` are retained next to the application.
After a successful update, the backup can be kept for manual recovery; remove
old helper/backup files only while Wisp is closed. There is no background cleanup
service. A power failure during replacement may require restoring the backup
manually. Application data in `%APPDATA%\Wisp` is not replaced or migrated.
A downloaded update is currently staged for this running session; restarting
without installing requires checking/downloading again.

## Release assets

Every compatible stable release must include these **exact** asset names:

- `wisp-windows-x64.exe`
- `wisp-update.json`
- `wisp-update.json.sig`

The JSON is signed as exact bytes (do not edit or reformat it after signing):

```json
{"version":"0.2.1","size":1234567,"sha256":"<64 lowercase hex characters>"}
```

The signature file is a hex-encoded Ed25519 signature of those bytes. The
application embeds `update-public-key.hex`. An unsigned historical release is
reported as unsupported, never installed as a fallback. GitHub release notes
are displayed as a four-line plain-text preview. There are no Up/Down controls.
Manual checks have a persisted 60-second cooldown, including failed attempts;
the primary action is disabled while checking/downloading. This reduces repeated
requests but does not bypass GitHub rate limits or HTTP 403 responses.

v0.2.1 is the first release meant to carry these assets. Releases up to and
including v0.2.0 have no updater, so their users install 0.2.1 from the zip once;
later releases then arrive through Settings > Updates. Pushing a branch is not
publishing an update.

### Maintainer packaging

The initial signing seed is stored locally outside the repository at
`C:\Users\Administrator\AppData\Local\hermes\wisp-update-signing-key.hex`.
Do not upload that file, paste its contents into chat or commit it. Back it up
securely: losing it prevents signing updates accepted by existing installations.
Public-key rotation requires a deliberate migration in a trusted release.

1. Set the correct stable `[package].version` in Cargo.toml, matching the tag.
2. Build the release executable using the supported Windows toolchain.
3. Install Python `cryptography` in your packaging environment.
4. Set `WISP_UPDATE_SIGNING_KEY_FILE` to the private seed's path.
5. Run:

```text
python scripts/package-update.py --exe target/release/wisp.exe --version 0.2.1 --out dist/update-0.2.1
```

6. Upload all three generated assets to the matching `v0.2.1` stable Release,
   alongside the normal portable ZIP and changelog. Test before publishing.

The packaging script refuses a mismatched signing key, invalid version, invalid
PE architecture, and existing output files. It does not publish anything.
No GitHub Action or remote signing secret is installed automatically.
Windows Authenticode/SmartScreen reputation is separate from updater signing.

## User-run verification

The `updater::tests` unit tests pass (`cargo test`, 0.2.1). A real
download, replace and roll-back has not been run yet; do the checks below.

- Open Updates; check current version, auto-check toggle persistence and Back.
- Check while offline; confirm a useful error and responsive UI; retry online.
- A historical release without signed assets must be rejected.
- For end-to-end testing, use a disposable writable application directory and
  isolated APPDATA, then publish a deliberately prepared newer signed test
  release only after approval. Do not test replacement against the only copy.
- Download, cancel and retry. A modified manifest/signature/executable must fail.
- Restart/install; verify the new executable, normal app startup and backup.
- Exercise a failed startup and locked/read-only destination; confirm recovery.
- Verify settings, tasks, calendar, shortcuts and shelf remain unchanged.

Targeted unit tests (no live media integration):

```text
cargo test --locked updater::tests -- --test-threads=1
```

Do not call the updater production-ready before the replacement/rollback paths
have been exercised on Windows. No CI or full runtime success is claimed.
