"""Assemble an isolated Windows runtime from the pinned archives, without pip.

Development tool; not invoked by the installed application. Never executes wheel
installation hooks. The model is deliberately outside this runtime.
"""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import stat
import tarfile
import zipfile


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def checked_target(root, name):
    # Reject Windows paths even when tests run on Unix.
    path = PurePosixPath(name)
    if not name or "\\" in name or ":" in name or path.is_absolute() or ".." in path.parts:
        raise ValueError("unsafe archive path")
    target = root.joinpath(*path.parts)
    if not target.resolve().is_relative_to(root.resolve()):
        raise ValueError("archive path escapes destination")
    return target


def extract_zip(source, root):
    with zipfile.ZipFile(source) as archive:
        for entry in archive.infolist():
            target = checked_target(root, entry.filename)
            if stat.S_ISLNK(entry.external_attr >> 16):
                raise ValueError("archive contains a symlink")
            if entry.is_dir():
                target.mkdir(parents=True, exist_ok=True)
                continue
            target.parent.mkdir(parents=True, exist_ok=True)
            with archive.open(entry) as src, target.open("xb") as dst:
                shutil.copyfileobj(src, dst, 1024 * 1024)


def assemble(manifest, downloads, destination):
    if destination.exists():
        raise ValueError("destination must not already exist")
    # Verify everything before extraction, including the executable interpreter.
    for artifact in manifest["artifacts"]:
        source = checked_target(downloads, artifact["file"])
        if source.stat().st_size != artifact["bytes"] or digest(source) != artifact["sha256"]:
            raise ValueError("artifact verification failed: " + artifact["name"])
    staging = destination.with_name(destination.name + ".staging")
    staging.mkdir(parents=True, exist_ok=False)
    try:
        site = staging / "Lib" / "site-packages"
        for artifact in manifest["artifacts"]:
            source = downloads / artifact["file"]
            if artifact["kind"] == "python":
                extract_zip(source, staging)
            elif artifact["kind"] == "wheel":
                extract_zip(source, site)
            elif artifact["kind"] == "docopt":
                # The upstream distribution has no wheel. Copy the module and
                # license explicitly; never execute setup.py or extract links.
                with tarfile.open(source) as archive:
                    for member, name in (("docopt.py", "docopt.py"), ("LICENSE-MIT", "docopt-LICENSE-MIT.txt")):
                        entry = archive.getmember("docopt-0.6.2/" + member)
                        if not entry.isfile():
                            raise ValueError("invalid docopt member")
                        with archive.extractfile(entry) as src, (site / name).open("xb") as dst:
                            shutil.copyfileobj(src, dst)
            else:
                raise ValueError("unknown artifact kind")
        (staging / "python311._pth").write_text("python311.zip\n.\nLib/site-packages\n", encoding="ascii")
        inventory = {str(p.relative_to(staging)).replace("\\", "/"): digest(p)
                     for p in sorted(staging.rglob("*")) if p.is_file()}
        (staging / "runtime.json").write_text(json.dumps({"id": manifest["id"], "files": inventory}, indent=2), encoding="utf-8")
        staging.rename(destination)
    except BaseException:
        # Only the unique staging directory created above belongs to this run.
        shutil.rmtree(staging)
        raise


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--downloads", type=Path, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    manifest = json.loads(Path(__file__).with_name("runtime-manifest.json").read_text(encoding="utf-8"))
    assemble(manifest, args.downloads.resolve(), args.destination.resolve())
    print(json.dumps({"runtime": manifest["id"], "destination": str(args.destination)}))
