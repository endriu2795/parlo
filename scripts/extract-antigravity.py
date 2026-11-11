"""Extract only the two expected files from a verified official Google archive."""
import pathlib
import shutil
import stat
import sys
import zipfile

root = pathlib.Path(sys.argv[1]).resolve()
expected = set(sys.argv[2:4])
with zipfile.ZipFile(root / "runtime.zip") as zipped:
    entries = zipped.infolist()
    if {e.filename for e in entries} != expected or len(entries) != 2:
        raise ValueError("Unexpected Antigravity archive layout")
    if sum(e.file_size for e in entries) > 700_000_000:
        raise ValueError("Archive too large")
    for entry in entries:
        if pathlib.PurePosixPath(entry.filename).name != entry.filename or stat.S_ISLNK(entry.external_attr >> 16):
            raise ValueError("Unsafe archive entry")
        target = root / entry.filename
        with zipped.open(entry) as source, target.open("wb") as output:
            shutil.copyfileobj(source, output)
        target.chmod(0o755)
