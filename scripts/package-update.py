"""Create HTTPS + SHA-256 portable updater assets; never uploads or publishes a release.
Requires Python 3.11+; no signing key or third-party packages needed.
Usage: python scripts/package-update.py --exe target/release/wisp.exe --version 0.2.1 --out dist/update
"""
import argparse
import hashlib
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--exe', type=Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    parts = args.version.split('.')
    if len(parts) != 3 or any(not p.isascii() or not p.isdigit() or str(int(p)) != p for p in parts):
        parser.error('version must be a canonical stable major.minor.patch')
    repo = Path(__file__).resolve().parents[1]
    import tomllib
    package_version = tomllib.loads((repo / 'Cargo.toml').read_text())['package']['version']
    if args.version != package_version:
        parser.error('version must match Cargo.toml; rebuild after changing the version')
    if args.exe.stat().st_mtime < (repo / 'Cargo.toml').stat().st_mtime:
        parser.error('executable predates Cargo.toml; rebuild before packaging')
    exe = args.exe.read_bytes()
    if not 4096 <= len(exe) <= 100 * 1024 * 1024 or exe[:2] != b'MZ':
        parser.error('expected a Windows executable, between 4 KiB and 100 MiB')
    pe = int.from_bytes(exe[0x3c:0x40], 'little')
    if exe[pe:pe + 6] != b'PE\0\0\x64\x86':
        parser.error('expected an x64 PE executable')
    manifest = json.dumps({'version': args.version, 'size': len(exe), 'sha256': hashlib.sha256(exe).hexdigest()}, separators=(',', ':')).encode()
    args.out.mkdir(parents=True, exist_ok=True)
    for name, data in [('wisp-windows-x64.exe', exe), ('wisp-update.json', manifest)]:
        path = args.out / name
        if path.exists():
            parser.error(f'refusing to overwrite {path}')
        path.write_bytes(data)
    print(f'Created two updater assets in {args.out.resolve()}. Nothing published.')
    print('Upload both to the matching stable GitHub Release. Build/package version must match.')


if __name__ == '__main__':
    main()
