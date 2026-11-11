"""Build-time resource preparation, sharing the app's pinned artifact manifest."""
import argparse
import hashlib
import json
from pathlib import Path
import uuid
from urllib.request import urlopen


def bundle_python(platform):
    root = Path(__file__).resolve().parents[1]
    manifest = json.loads((root / 'engine/python-runtime.json').read_text())
    artifact = next(a for a in manifest['artifacts'] if a['platform'] == platform)
    folder = root / 'engine/runtime/python'
    folder.mkdir(parents=True, exist_ok=True)
    target = folder / artifact['file']

    def valid(path):
        if not path.is_file() or path.stat().st_size != artifact['size']:
            return False
        with path.open('rb') as source:
            return hashlib.file_digest(source, 'sha256').hexdigest() == artifact['sha256']

    if not valid(target):
        partial = folder / f"{artifact['file']}.{uuid.uuid4()}.part"
        try:
            with urlopen(artifact['url'], timeout=120) as response, partial.open('wb') as output:
                count = 0
                while chunk := response.read(65536):
                    count += len(chunk)
                    if count > artifact['size']:
                        raise RuntimeError('Dimensione del runtime Python inattesa.')
                    output.write(chunk)
            if not valid(partial):
                raise RuntimeError('Checksum o dimensione del runtime Python non validi.')
            partial.replace(target)
        finally:
            partial.unlink(missing_ok=True)
    print(f'Python incluso: {target}')
    return target


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--platform', choices=['win-x64', 'mac-arm64', 'mac-x64'], required=True)
    bundle_python(parser.parse_args().platform)
