"""Generate and attach wisp-update.json to a draft GitHub Release.

The Release workflow now attaches wisp-update.json automatically for stable tags,
so this script is kept as a manual fallback: it downloads wisp-windows-x64.exe from
the draft Release, generates wisp-update.json via package-update.py (HTTPS + SHA-256),
and uploads it next to the exe. It never publishes; check the draft on GitHub, add
the notes, then publish it yourself.

Requires: gh (signed in), Python 3.11+ (stdlib only), and a checkout whose Cargo.toml
has the release's version (package-update.py checks it).
Usage: python scripts/sign-release.py v0.2.2
"""
import subprocess
import sys
import tempfile
from pathlib import Path


def run(*args):
    subprocess.run(args, check=True)


def main():
    if len(sys.argv) != 2 or not sys.argv[1].startswith('v'):
        sys.exit(__doc__)
    tag = sys.argv[1]
    version = tag[1:]
    repo = Path(__file__).resolve().parents[1]
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        run('gh', 'release', 'download', tag, '--pattern', 'wisp-windows-x64.exe', '--dir', str(tmp))
        out = tmp / 'update'
        run(sys.executable, str(repo / 'scripts' / 'package-update.py'),
            '--exe', str(tmp / 'wisp-windows-x64.exe'), '--version', version, '--out', str(out))
        run('gh', 'release', 'upload', tag, '--clobber', str(out / 'wisp-update.json'))
    print(f'Updater manifest (wisp-update.json) attached to the {tag} draft. Review it on GitHub, then publish.')


if __name__ == '__main__':
    main()
