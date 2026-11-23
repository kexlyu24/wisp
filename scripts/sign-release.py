"""Sign the exe that the Release workflow built and attach the updater files to its draft.

The private key stays on this PC: this downloads wisp-windows-x64.exe from the draft Release,
signs it with package-update.py, and uploads wisp-update.json and wisp-update.json.sig next to
it. It never publishes; check the draft on GitHub, add the notes, then publish it yourself.

Requires: gh (signed in), python -m pip install cryptography,
WISP_UPDATE_SIGNING_KEY_FILE as for package-update.py, and a checkout whose Cargo.toml has the
release's version (package-update.py checks it).
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
        out = tmp / 'signed'
        run(sys.executable, str(repo / 'scripts' / 'package-update.py'),
            '--exe', str(tmp / 'wisp-windows-x64.exe'), '--version', version, '--out', str(out))
        run('gh', 'release', 'upload', tag, '--clobber',
            str(out / 'wisp-update.json'), str(out / 'wisp-update.json.sig'))
    print(f'Signed updater files attached to the {tag} draft. Review it on GitHub, then publish.')


if __name__ == '__main__':
    main()
