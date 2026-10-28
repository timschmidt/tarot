#!/usr/bin/env python3
"""Normalize APK2 script checksum names for APK3's stricter tar path validation.

APK2 embeds base64 SHA1 digests in script names. A digest containing // becomes
an invalid tar pathname to APK3. Both APK versions accept X1-prefixed hexadecimal
SHA1 digests. Rewrite only those offending names; preserve scripts and metadata.
This prepares a separate output archive and never edits an installed database.
"""

import argparse
import base64
import io
from pathlib import Path
import tarfile


def repair(source: Path, destination: Path) -> list[tuple[str, str]]:
    if source.resolve() == destination.resolve():
        raise ValueError("source and destination must be different files")
    changes = []
    entries = []
    with tarfile.open(source, "r:*") as archive:
        for member in archive.getmembers():
            if not member.isfile():
                raise ValueError(f"unexpected nonregular script entry: {member.name}")
            data = archive.extractfile(member).read()
            name = member.name
            if "//" in name:
                prefix, checksum, action = name.rsplit(".", 2)
                if not checksum.startswith("Q1") or action not in {
                    "pre-install", "post-install", "pre-upgrade", "post-upgrade",
                    "pre-deinstall", "post-deinstall", "trigger",
                }:
                    raise ValueError(f"unrecognized APK script name: {name}")
                digest = base64.b64decode(checksum[2:], validate=True)
                if len(digest) != 20:
                    raise ValueError(f"invalid SHA1 checksum: {name}")
                member.name = f"{prefix}.X1{digest.hex()}.{action}"
                changes.append((name, member.name))
            if any(part in ("", ".", "..") for part in member.name.split("/")):
                raise ValueError(f"unsafe script name: {member.name}")
            entries.append((member, data))

    if len({member.name for member, _ in entries}) != len(entries):
        raise ValueError("duplicate script names after normalization")
    with destination.open("xb") as output:
        with tarfile.open(fileobj=output, mode="w", format=tarfile.USTAR_FORMAT) as archive:
            for member, data in entries:
                archive.addfile(member, io.BytesIO(data))
    return changes


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    for old, new in repair(args.source, args.destination):
        print(f"{old} -> {new}")
