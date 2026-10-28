import base64
import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest

MODULE_PATH = Path(__file__).resolve().parents[1] / "scripts/repair-apk-v2-scripts.py"
SPEC = importlib.util.spec_from_file_location("apk_repair", MODULE_PATH)
REPAIR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(REPAIR)


class ApkScriptRepairTests(unittest.TestCase):
    def test_real_rejected_digest_preserves_identity_and_script_metadata(self):
        digest = "Wl01iED7mgxAeX5XjpKbF//nMqU="
        name = f"device-mapper-udev-2.03.32-r0.Q1{digest}.post-install"
        payload = b"#!/bin/sh\nexit 0\n"
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "old.tar"
            destination = Path(directory) / "new.tar"
            member = tarfile.TarInfo(name)
            member.size = len(payload)
            member.mode = 0o755
            member.uid = 12
            member.gid = 34
            member.mtime = 12345
            with tarfile.open(source, "w") as archive:
                archive.addfile(member, io.BytesIO(payload))
            changes = REPAIR.repair(source, destination)
            self.assertEqual(len(changes), 1)
            with tarfile.open(destination) as archive:
                repaired = archive.getmembers()[0]
                checksum = repaired.name.rsplit(".", 2)[1]
                self.assertTrue(checksum.startswith("X1"))
                self.assertEqual(bytes.fromhex(checksum[2:]), base64.b64decode(digest))
                self.assertEqual(archive.extractfile(repaired).read(), payload)
                self.assertEqual((repaired.mode, repaired.uid, repaired.gid, repaired.mtime), (0o755, 12, 34, 12345))

    def test_unrelated_path_traversal_is_rejected_without_an_output_archive(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "old.tar"
            destination = Path(directory) / "new.tar"
            with tarfile.open(source, "w") as archive:
                archive.addfile(tarfile.TarInfo("../outside"), io.BytesIO())
            with self.assertRaises(ValueError):
                REPAIR.repair(source, destination)
            self.assertFalse(destination.exists())


if __name__ == "__main__":
    unittest.main()
