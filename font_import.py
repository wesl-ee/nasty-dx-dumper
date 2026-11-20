#!/usr/bin/env python3
"""
Write data/font00.png over glyph page 0 of the font main.dol links in.

The DOL carries its own copy of font00.tpl and draws from that, not the disc
file, so the retail page is found in the DOL by content.

Usage:
  python font_import.py [iso_dir] [out_dir]
"""

import struct
import sys
from pathlib import Path

from PIL import Image
from gclib.texture_utils import ImageFormat, PaletteFormat, encode_image

iso = Path(sys.argv[1] if len(sys.argv) > 1 else "dx-iso-base")
dol = Path(sys.argv[2] if len(sys.argv) > 2 else "tl-output") / "main.dol"

tpl = (iso / "files/data/font00.tpl").read_bytes()
# the image table is at 0x0c and page 0 is its first entry
h, w, _, data = struct.unpack_from(">HHII", tpl, struct.unpack_from(">I", tpl, 12)[0])
old = tpl[data : data + w * h // 2]
new = encode_image(Image.open("data/font00.png"), ImageFormat.I4, PaletteFormat.IA8)[0].getvalue()

buf = (dol if dol.exists() else iso / "sys/main.dol").read_bytes()
if new not in buf:
    at = buf.index(old)
    dol.write_bytes(buf[:at] + new + buf[at + len(old) :])
