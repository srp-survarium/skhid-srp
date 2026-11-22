"""Content-addressed prepared inputs, independent of their local directory names."""
import hashlib
from pathlib import Path


def fingerprint(root):
    digest = hashlib.sha256()
    count = 0
    for path in sorted(Path(root).rglob('*')):
        if not path.is_file():
            continue
        relative = str(path.relative_to(root))
        digest.update(relative.encode() + b'\0')
        with path.open('rb') as stream:
            digest.update(hashlib.file_digest(stream, 'sha256').digest())
        count += 1
    return dict(files=count, sha256=digest.hexdigest())
