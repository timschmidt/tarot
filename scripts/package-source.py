#!/usr/bin/env python3
"""Stage a deterministic, vendored source archive and checksummed APKBUILD.

First run `cargo vendor --locked --versioned-dirs /absolute/vendor`. The supplied
vendor directory must contain the exact registry packages in this Cargo.lock.
No build outputs, device evidence, git data or user Cargo credentials are packed.
"""
import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import tarfile
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--vendor', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parent.parent
    vendor = args.vendor.resolve()
    out = args.output.resolve()
    roots = ['src', 'crates', 'examples', 'third-party', 'scripts', 'packaging', 'tests', 'docs']
    assert vendor.is_dir() and not out.is_relative_to(vendor)
    assert not any(out.is_relative_to(repo/name) for name in roots), 'Stage outside packaged source directories'
    out.mkdir(parents=True, exist_ok=True)
    version = tomllib.loads((repo/'Cargo.toml').read_text())['package']['version']
    packages = [p for p in tomllib.loads((repo/'Cargo.lock').read_text())['package'] if p.get('source', '').startswith('registry+')]
    expected = {f"{p['name']}-{p['version']}": p for p in packages}
    assert {p.name for p in vendor.iterdir()} == set(expected), 'Vendor exactly the current lockfile'
    for name, package in expected.items():
        directory = vendor/name
        metadata = directory/'.cargo-checksum.json'
        assert directory.is_dir() and not directory.is_symlink() and not metadata.is_symlink()
        checksum = json.loads(metadata.read_text())
        assert checksum['package'] == package['checksum'], name
        actual = set()
        for path in directory.rglob('*'):
            assert not path.is_symlink(), path
            if path.is_file():
                actual.add(path.relative_to(directory).as_posix())
        assert actual == set(checksum['files']) | {'.cargo-checksum.json'}, ('Unrecorded or missing vendor file', name)
        for rel, digest in checksum['files'].items():
            path = vendor/name/rel
            assert not path.is_symlink() and path.is_file() and path.resolve().is_relative_to(vendor/name)
            assert hashlib.sha256(path.read_bytes()).hexdigest() == digest, (name, rel)
    archive = out/f'tarot-{version}.tar.gz'
    prefix = f'tarot-{version}'
    epoch = int(os.environ.get('SOURCE_DATE_EPOCH', '0'))
    files = [repo/name for name in ['Cargo.toml', 'Cargo.lock', 'README.md', 'LICENSE-MIT', 'LICENSE-APACHE']]
    for name in roots:
        files.extend(p for p in (repo/name).rglob('*') if p.is_file() and not any(part in ['target', '.git', '__pycache__'] for part in p.relative_to(repo).parts))
    config = (repo/'.cargo/config.toml').read_text()+'\n[source.crates-io]\nreplace-with = "vendored-sources"\n\n[source.vendored-sources]\ndirectory = "vendor"\n'
    sources = {}
    with archive.open('xb') as raw, gzip.GzipFile(filename='', fileobj=raw, mode='wb', mtime=epoch, compresslevel=3) as compressed, tarfile.open(fileobj=compressed, mode='w') as tar:
        def add(name, data, executable=False):
            info = tarfile.TarInfo(prefix+'/'+name)
            info.size = len(data)
            info.mode = 0o755 if executable else 0o644
            info.mtime = epoch
            tar.addfile(info, io.BytesIO(data))
        for path in sorted(files):
            assert not path.is_symlink() and path.resolve().is_relative_to(repo)
            rel = path.relative_to(repo).as_posix()
            data = path.read_bytes()
            sources[rel] = hashlib.sha256(data).hexdigest()
            add(rel, data, bool(path.stat().st_mode & 0o111))
        add('.cargo/config.toml', config.encode())
        for path in sorted(vendor.rglob('*')):
            if path.is_file():
                assert not path.is_symlink()
                add('vendor/'+path.relative_to(vendor).as_posix(), path.read_bytes(), bool(path.stat().st_mode & 0o111))
    data = archive.read_bytes()
    checksum = hashlib.sha512(data).hexdigest()
    recipe = (repo/'packaging/nura/APKBUILD').read_text()+f'\nsha512sums="\n{checksum}  {archive.name}\n"\n'
    (out/'APKBUILD').write_text(recipe)
    record = {'version': version, 'epoch': epoch, 'source_sha256': hashlib.sha256(data).hexdigest(), 'source_sha512': checksum, 'registry_package_count': len(packages), 'project_source_sha256': sources, 'apkbuild_sha256': hashlib.sha256(recipe.encode()).hexdigest()}
    (out/'source-manifest.json').write_text(json.dumps(record, indent=2)+'\n')
    print(f'Staged {archive.name}: {len(packages)} locked registry packages, SHA-256 {record["source_sha256"]}')


if __name__ == '__main__':
    main()
