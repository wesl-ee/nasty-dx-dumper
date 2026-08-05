iso := '../DokaponDX [GDNJE8] [96804941][b].iso'
eng := './out/DokaponDX [GDNJE8] [96804941][b].eng.iso'

default:
    @just --list

extract-iso:
    wit extract --overwrite '{{iso}}' ./dx-iso-base

# regenerates every sidecar from scratch: wipes translation progress
dump:
    cargo build
    ./target/debug/nasty_dx_dumper dump-all ./tl

patch:
    cargo build
    ./target/debug/nasty_dx_dumper patch-all ./tl ./tl-output

build-iso: patch
    python build_iso.py '{{iso}}' ./tl-output '{{eng}}'

# large source window because retail iso layout is different than that produced
# by gcft
xdelta:
    xdelta3 -e -9 -B 1459978240 -s '{{iso}}' '{{eng}}' dokapon-en.xdelta

xlsx-extract:
    python xlsx_tl.py extract ./tl tl.xlsx

xlsx-merge sheet:
    python xlsx_tl.py merge ./tl '{{sheet}}' ./tl-merged
    cp -r tl-merged/. tl/
