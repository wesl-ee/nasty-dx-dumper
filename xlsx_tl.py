#!/usr/bin/env python3
"""
Produce an xlsx for collaborative TL work

extract: one worksheet per <dir>/**/*.patch file, columns
         address | Original | EN Translation
merge:   reapply EN Translation from the workbook back into the .patch
         files

Usage:
  python xlsx_tl.py extract <dir> <out.xlsx>
  python xlsx_tl.py merge <dir> <sheet.xlsx> <out_dir>
"""

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

from openpyxl import Workbook, load_workbook
from openpyxl.styles import Alignment

WRAP = Alignment(wrap_text=True, vertical="top")

ENTRY_RE = re.compile(r"^0x([0-9a-f]+) (.*)$")
INVALID_SHEET_CHARS = re.compile(r"[:\\/?*\[\]]")


@dataclass
class Line:
    kind: str  # "verbatim" | "jp" | "en"
    address: Optional[int]
    text: str


def parse_patch(path: Path) -> list[Line]:
    lines: list[Line] = []
    block_addr = None
    # a .patch file always ends with a trailing newline, so split("\n")
    # leaves one phantom empty element at the end; drop it
    for raw in path.read_text(encoding="utf-8").split("\n")[:-1]:
        if raw.strip() == "":
            block_addr = None
            lines.append(Line("verbatim", None, raw))
            continue

        m = ENTRY_RE.match(raw)
        if not m:
            lines.append(Line("verbatim", None, raw))
            continue

        addr, text = int(m.group(1), 16), m.group(2)
        if block_addr is None:
            lines.append(Line("jp", addr, text))
            block_addr = addr
        elif addr == block_addr:
            lines.append(Line("en", addr, text))
        else:
            raise ValueError(
                f"{path}: {raw!r} has address 0x{addr:x} but its block's "
                f"JP address is 0x{block_addr:x} (missing blank line separator?)"
            )
    return lines


def write_patch(path: Path, lines: list[Line]) -> None:
    with path.open("w", encoding="utf-8", newline="\n") as f:
        for l in lines:
            if l.kind == "verbatim":
                f.write(l.text + "\n")
            else:
                f.write(f"0x{l.address:x} {l.text}\n")


def sheet_name(rel_path: Path) -> str:
    stem = str(rel_path.with_suffix(""))  # drop the trailing .patch
    return INVALID_SHEET_CHARS.sub("_", stem)[:31]


def patch_files(dir_: Path):
    for path in sorted(dir_.rglob("*")):
        if path.is_file() and path.suffix.lower() == ".patch":
            yield path, path.relative_to(dir_)


def extract(dir_: Path, out_xlsx: Path) -> None:
    wb = Workbook()
    wb.remove(wb.active)

    # main.dol.patch is the largest file, so it leads the per-file sheets;
    # files with no translatable lines get no sheet at all
    entries = [(p, rel, parse_patch(p)) for p, rel in patch_files(dir_)]
    entries = [e for e in entries if any(l.kind == "jp" for l in e[2])]
    entries.sort(key=lambda e: (e[1] != Path("main.dol.patch"), str(e[1])))

    status_rows = []
    for path, rel, doc in entries:
        name = sheet_name(rel)
        if name in wb.sheetnames:
            raise SystemExit(f"{path}: sheet name {name!r} collides with an earlier file")
        ws = wb.create_sheet(name)
        ws.append(["address", "Original", "EN Translation"])
        ws.column_dimensions["A"].width = 10
        ws.column_dimensions["B"].width = 60
        ws.column_dimensions["C"].width = 60

        total = 0
        for i, line in enumerate(doc):
            if line.kind != "jp":
                continue
            target_en = None
            nxt = doc[i + 1] if i + 1 < len(doc) else None
            if nxt is not None and nxt.kind == "en" and nxt.address == line.address:
                target_en = nxt.text
            ws.append([f"0x{line.address:x}", line.text, target_en])
            ws.cell(row=ws.max_row, column=2).alignment = WRAP
            ws.cell(row=ws.max_row, column=3).alignment = WRAP
            total += 1

        status_rows.append((name, total))

    notes = wb.create_sheet("Notes", 0)
    notes.append(["File", "Total", "Translated", "% Complete", "Notes"])
    notes.column_dimensions["A"].width = 24
    notes.column_dimensions["E"].width = 60
    for name, total in status_rows:
        notes.append([name, total, f"=COUNTA('{name}'!C2:C{total + 1})", None, None])
        r = notes.max_row
        notes.cell(row=r, column=4).value = f"=C{r}/B{r}"
        notes.cell(row=r, column=4).number_format = "0%"

    wb.save(out_xlsx)
    print(out_xlsx)


def apply_sheet(
    doc: list[Line], sheet_map: dict[int, str], path: Path
) -> tuple[list[Line], int]:
    out: list[Line] = []
    i = 0
    updated = 0
    while i < len(doc):
        line = doc[i]
        if line.kind != "jp":
            out.append(line)
            i += 1
            continue

        target_en = sheet_map.get(line.address)
        nxt = doc[i + 1] if i + 1 < len(doc) else None
        has_en = nxt is not None and nxt.kind == "en" and nxt.address == line.address

        out.append(line)
        if has_en:
            # the sheet is the source of truth: an edit there overwrites
            # whatever is already in the file, including prior merges
            if target_en and target_en != nxt.text:
                print(
                    f"{path}: 0x{line.address:x} updated from sheet: "
                    f"{nxt.text!r} -> {target_en!r}",
                    file=sys.stderr,
                )
                out.append(Line("en", line.address, target_en))
                updated += 1
            else:
                out.append(nxt)
            i += 2
        elif target_en:
            out.append(Line("en", line.address, target_en))
            i += 1
        else:
            i += 1
    return out, updated


def load_sheet_map(ws, sheet_xlsx: Path, name: str) -> dict[int, str]:
    sheet_map: dict[int, str] = {}
    for row in ws.iter_rows(min_row=2, values_only=True):
        # read_only worksheets trim a row's tuple to its last non-empty
        # cell, so a blank trailing row comes back as () and a row whose
        # only content is column A comes back as length 1
        if not row or row[0] is None:
            continue
        addr = int(str(row[0]).strip().removeprefix("0x"), 16)
        if addr in sheet_map:
            raise SystemExit(f"{sheet_xlsx}:{name}: duplicate address 0x{addr:x}")
        sheet_map[addr] = (row[2] if len(row) > 2 else None) or ""
    return sheet_map


def merge(dir_: Path, sheet_xlsx: Path, out_dir: Path) -> None:
    wb = load_workbook(sheet_xlsx, read_only=True, data_only=True)
    total_updated = 0

    for path, rel in patch_files(dir_):
        doc = parse_patch(path)
        if not any(l.kind == "jp" for l in doc):
            continue

        name = sheet_name(rel)
        if name not in wb.sheetnames:
            # the megasheet is a work-in-progress snapshot; files dumped
            # since the last extract have no sheet to merge yet
            print(f"{path}: no sheet {name!r} in workbook, skipping", file=sys.stderr)
            continue

        sheet_map = load_sheet_map(wb[name], sheet_xlsx, name)

        file_addrs = {l.address for l in doc if l.kind == "jp"}
        # a row the translator deleted (untranslatable, pulled from memory,
        # etc.) is normal; a sheet row addressing text no longer in the file
        # means the sheet and the dump have genuinely diverged
        missing_from_sheet = file_addrs - sheet_map.keys()
        missing_from_file = sheet_map.keys() - file_addrs
        if missing_from_sheet:
            print(
                f"{path}: {len(missing_from_sheet)} address(es) in file but not "
                f"sheet (no row to merge): {sorted(hex(a) for a in missing_from_sheet)}",
                file=sys.stderr,
            )
        if missing_from_file:
            raise SystemExit(
                f"{path}: sheet addresses no longer in file: "
                f"{sorted(hex(a) for a in missing_from_file)}"
            )

        out_path = out_dir / rel
        out_path.parent.mkdir(parents=True, exist_ok=True)
        merged, updated = apply_sheet(doc, sheet_map, path)
        write_patch(out_path, merged)
        total_updated += updated
        print(out_path)

    if total_updated:
        print(f"{total_updated} existing line(s) overwritten from the sheet", file=sys.stderr)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)

    ex = sub.add_parser("extract")
    ex.add_argument("dir", type=Path)
    ex.add_argument("out_xlsx", type=Path)

    mg = sub.add_parser("merge")
    mg.add_argument("dir", type=Path)
    mg.add_argument("sheet_xlsx", type=Path)
    mg.add_argument("out_dir", type=Path)

    args = ap.parse_args()
    if args.cmd == "extract":
        extract(args.dir, args.out_xlsx)
    else:
        merge(args.dir, args.sheet_xlsx, args.out_dir)


if __name__ == "__main__":
    main()
