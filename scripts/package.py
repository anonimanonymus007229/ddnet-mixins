#!/usr/bin/env python3
"""Package an already built native ddmix executable; no build/download side effects."""
import argparse
import hashlib
import pathlib
import zipfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--binary", type=pathlib.Path, required=True)
parser.add_argument("--target", required=True)
parser.add_argument("--out", type=pathlib.Path, default=pathlib.Path("target/dist"))
args = parser.parse_args()
if not args.binary.is_file():
    parser.error("binary does not exist; run cargo build --release first")
if not args.target or any(c not in "abcdefghijklmnopqrstuvwxyz0123456789_-" for c in args.target):
    parser.error("invalid target label")
root = pathlib.Path(__file__).resolve().parents[1]
args.out.mkdir(parents=True, exist_ok=True)
archive = args.out / f"ddmix-{args.target}.zip"
with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as z:
    z.write(args.binary, args.binary.name)
    z.write(root / "LICENSE", "LICENSE")
    z.write(root / "README.md", "README.md")
digest = hashlib.sha256(archive.read_bytes()).hexdigest()
archive.with_suffix(".zip.sha256").write_text(f"{digest}  {archive.name}\n", encoding="utf-8")
print(archive)
