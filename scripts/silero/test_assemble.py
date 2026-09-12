"""No network, model or PyTorch required."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location("assemble", Path(__file__).with_name("assemble.py"))
assembler = importlib.util.module_from_spec(spec)
spec.loader.exec_module(assembler)


class AssemblyTests(unittest.TestCase):
    def test_rejects_path_traversal_and_windows_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            for name in ("../escape", "/absolute", "C:/absolute", "dir\\escape", "dir/../../escape", "file:stream"):
                with self.subTest(name=name), self.assertRaises(ValueError):
                    assembler.checked_target(Path(tmp), name)

    def test_corrupt_archive_never_activates(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "bad.zip").write_bytes(b"bad")
            manifest = {"id": "test", "artifacts": [{"file": "bad.zip", "name": "bad", "bytes": 3, "sha256": "0" * 64}]}
            with self.assertRaises(ValueError):
                assembler.assemble(manifest, root, root / "runtime")
            self.assertFalse((root / "runtime").exists())
            self.assertFalse((root / "runtime.staging").exists())

    def test_failed_extraction_cleans_staging_and_preserves_existing_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            sentinel = root / "keep.txt"
            sentinel.write_text("keep")
            archive = root / "bad.zip"
            with zipfile.ZipFile(archive, "w") as writer:
                writer.writestr("ok.txt", "ok")
                writer.writestr("../keep.txt", "overwrite")
            manifest = {"id": "test", "artifacts": [{"file": archive.name, "name": "python", "kind": "python", "bytes": archive.stat().st_size, "sha256": assembler.digest(archive)}]}
            with self.assertRaises(ValueError):
                assembler.assemble(manifest, root, root / "runtime")
            self.assertEqual(sentinel.read_text(), "keep")
            self.assertFalse((root / "runtime.staging").exists())

    def test_verified_runtime_has_inventory_and_isolated_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = root / "python.zip"
            with zipfile.ZipFile(archive, "w") as writer:
                writer.writestr("LICENSE.txt", "license")
            manifest = {"id": "test", "artifacts": [{"file": archive.name, "name": "python", "kind": "python", "bytes": archive.stat().st_size, "sha256": assembler.digest(archive)}]}
            assembler.assemble(manifest, root, root / "runtime")
            inventory = json.loads((root / "runtime/runtime.json").read_text())
            self.assertEqual(inventory["files"]["LICENSE.txt"], assembler.digest(root / "runtime/LICENSE.txt"))
            self.assertNotIn("import site", (root / "runtime/python311._pth").read_text())
            with self.assertRaises(ValueError):
                assembler.assemble(manifest, root, root / "runtime")


if __name__ == "__main__":
    unittest.main()
