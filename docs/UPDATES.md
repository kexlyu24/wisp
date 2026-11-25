# In-app updates: HTTPS + SHA-256

Settings > Updates checks the latest stable GitHub Release in kexlyu24/wisp.
Drafts and prereleases are ignored. Automatic startup checks are optional and
run at most daily. Manual checks have no cooldown; the button is disabled only while a check or
download is in progress. Download and installation require explicit user actions.

## Trust model

Update metadata and executable downloads use HTTPS and the Windows trust store.
Asset URLs must match the official repository and release tag. Wisp checks the
manifest version, bounded file size, SHA-256 and Windows x64 PE header, and
rechecks the staged payload before installation. No signing key or signature
asset is required.

SHA-256 detects differences from the manifest; it does not authenticate the
publisher independently. Anyone who compromises the repository/release account
can replace both the executable and manifest. Local staging checks also cannot
authenticate a manifest and payload that a local attacker replaces together.
Protect release access with strong authentication. This is an intentional
trade-off compared with the previous Ed25519 design.

HTTP 403 / GitHub API limits are unchanged by this migration. Do not embed a
personal GitHub token in distributed executables.

## Release assets and packaging

Upload these exact names alongside the normal portable ZIP:

- wisp-windows-x64.exe
- wisp-update.json

The manifest contains version, size in bytes, and a 64-character SHA-256:

```json
{"version":"0.2.1","size":1234567,"sha256":"<64 lowercase hex characters>"}
```

Set Cargo.toml to the intended stable version and build a fresh Windows release.
Use Python 3.11+ (standard library only):

```text
python scripts/package-update.py --exe target/release/wisp.exe --version 0.2.1 --out dist/update-0.2.1
```

Use the actual package version in both arguments and the release tag. The script
rejects mismatched versions, stale executables, invalid PE architecture, and
existing output files. It creates two assets and never publishes anything.
There is no cryptography dependency, private-key environment variable, or public
key file. Extra historical .sig assets are ignored by this client.

## Migration from signature-based clients

Existing released clients that require Ed25519 signatures do not gain this
behavior by changing server metadata. Distribute this new client manually once,
or deliver it through the old signed pipeline using the key trusted by those
clients. Keep legacy signature assets for older clients if supporting that path.
The new packaging script does not produce legacy signatures. Never claim a
hash-only release will automatically upgrade a signature-requiring client.

## Installation and recovery

Download runs in a worker with progress, cancellation and retry. Cancellation
can take a network timeout while a synchronous request is blocked. Restart &
update closes Wisp, waits for the old process, replaces only wisp.exe, and starts
the new executable. A startup health marker must arrive within 15 seconds or the
helper attempts to restore and restart the previous executable. This is not
continuous crash monitoring. The executable must be named wisp.exe and its
folder must be writable; no elevation is requested. Run only one instance.

The helper and wisp-backup-<pid>.exe remain beside the executable for recovery.
A power failure may require restoring that backup manually. Settings and user
data in APPDATA/Wisp are not replaced. Error reporting uses a native message
and, where possible, APPDATA/Wisp/update-error.txt. A staged download belongs to
the current session; restarting without installing requires downloading again.

## Verification checklist

- Compile regression tests for valid/invalid manifests, versions, URLs and
  payload hash/size/PE headers.
- Package a freshly built executable without any key environment variable.
- Check offline errors, progress, cancellation and retry.
- In a disposable application folder and isolated APPDATA, test a deliberately
  prepared newer release: install, startup health, rollback and locked targets.
- Confirm tasks, calendar, settings and shelf survive installation.

Agent verification is compile/build and packaging only; interactive testing and
Rust test execution are left to the user. Do not claim install/rollback is
verified until exercised on Windows.
