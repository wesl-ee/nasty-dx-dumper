A dumper and patcher for Dokapon DX (GameCube)

```
wesl-ee@divinity ~/code/nasty-dx-dumper > md5sum 'DokaponDX [GDNJE8] [96804941][b].iso'
1cde120473a6c1ae54c7fde9164322b8  DokaponDX [GDNJE8] [96804941][b].iso

# though it should work on any version of the gamecube ROM...
```

### Extract the ISO

`wit` (wiimms-iso-tools) extracts files from the base ISO.

```sh
wit extract --overwrite '../DokaponDX [GDNJE8] [96804941][b].iso' ./dx-iso-base
```

### Dump (if dumping new files)

Only the DOL and the event scripts carry text, so `./tl` holds just those.
`dump-all` and `patch-all` mirror whatever tree shape you give them.

```sh
mkdir -p tl/Event
cp dx-iso-base/sys/main.dol tl/
cp dx-iso-base/files/Event/*.SPT dx-iso-base/files/Event/*.spt tl/Event/

# if using nix
nix develop --command cargo build

# otherwise build however you like
cargo build

./target/debug/nasty_dx_dumper dump-all ./tl
```

Each input gets a sidecar next to it: `tl/main.dol.patch`,
`tl/Event/BANK.SPT.patch`, and so on. Re-running `dump-all` regenerates them
from scratch and **wipes translation progress** — back the sidecars up first.

### Translate

A sidecar is comment lines, blank-line-separated blocks, and `0xOFFSET text`
entries. Translate by repeating the offset line with English underneath it:

```
0x2742 銀行員
0x2742 Teller
```

If using a spreadsheet see the section on
[translating with a spreadsheet](#translating-in-a-spreadsheet).

### Patch

```sh
./target/debug/nasty_dx_dumper patch-all ./tl ./tl-output
```

`patch-all` rewrites every reference to a string in both the DOL and SPT files.
The patching routine was informed with binary analysis done in Ghidra.

### Rebuild the ISO

`build_iso.py` reads the retail ISO, swaps in everything under `tl-output`, and
writes a new image with the FST relaid around the files that grew.

```sh
python build_iso.py '../DokaponDX [GDNJE8] [96804941][b].iso' ./tl-output \
    './out/DokaponDX [GDNJE8] [96804941][b].eng.iso'
```

`--trim` will prevent this script from padding the ISO to the full 1.46GiB.
Padding is needed so a USB loader can use. Dolphin doesn't care either way.

## Translating in a spreadsheet

```sh
# create a xlsx from .patch files
python xlsx_tl.py extract ./tl tl.xlsx

# import workbook as .patch files
python xlsx_tl.py merge ./tl ~/dl/tl.xlsx ./tl-merged
cp -r tl-merged/. tl/
```

## License

MIT License (available under /LICENSE)
