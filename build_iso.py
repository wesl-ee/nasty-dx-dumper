#!/usr/bin/env python3
"""
Rebuild a bootable GameCube image from patch-all's output.

Usage:
  python build_iso.py <retail.iso> <patched_dir> <out.iso>
"""

import argparse
import sys
from io import BytesIO
from pathlib import Path

from gclib.gcm import GCM

# a retail GameCube DVD is 1.46GiB and the drive reads in 32 byte units so every
# file on one starts 0x20 aligned
DISC_SIZE = 0x5705_8000
DVD_ALIGN = 0x20

_align = GCM.align_output_iso_to_nearest
GCM.align_output_iso_to_nearest = lambda self, size: _align(self, max(size, DVD_ALIGN))


def disc_path(gcm: GCM, rel: Path) -> str:
    # patch-all mirrors the tree it was given: main.dol at the root, event
    # scripts under Event/.  The DOL is one region the disc names twice
    for candidate in (f"files/{rel}", f"sys/{rel}"):
        if candidate.lower() in gcm.files_by_path_lowercase:
            return gcm.files_by_path_lowercase[candidate.lower()].file_path
    raise SystemExit(f"{rel}: no such file on the disc")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("iso", type=Path)
    ap.add_argument("patched_dir", type=Path)
    ap.add_argument("out_iso", type=Path)
    ap.add_argument(
        "--trim",
        action="store_true",
        help="stop at the last file instead of padding out to a full disc",
    )
    args = ap.parse_args()

    gcm = GCM(str(args.iso))
    gcm.read_entire_disc()

    for path in sorted(args.patched_dir.rglob("*")):
        if not path.is_file():
            continue
        rel = path.relative_to(args.patched_dir)
        data = path.read_bytes()
        targets = [disc_path(gcm, rel)]
        if targets[0] == "sys/main.dol":
            targets.append("files/default.dol")
        for target in targets:
            gcm.changed_files[target] = BytesIO(data)
        old = gcm.files_by_path[targets[0]].file_size
        print(f"{rel} -> {', '.join(targets)} ({old} -> {len(data)} bytes)")

    for _, files_done in gcm.export_disc_to_iso_with_changed_files(str(args.out_iso)):
        if files_done % 200 == 0:
            print(f"\r{files_done}/{len(gcm.files_by_path)} files", end="", file=sys.stderr)

    written = args.out_iso.stat().st_size
    if not args.trim:
        if written > DISC_SIZE:
            raise SystemExit(f"{written} bytes written, over the {DISC_SIZE} byte disc")
        # sparse: the tail reads back as zeros without costing the space
        with args.out_iso.open("r+b") as f:
            f.truncate(DISC_SIZE)
    print(f"\n{args.out_iso}: {written} bytes of data, {args.out_iso.stat().st_size} total")


if __name__ == "__main__":
    main()
