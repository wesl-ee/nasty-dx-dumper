Extract ISO using `wit`

```
wit extract --overwrite ../DokaponDX [GDNJE8] [96804941][b].iso ./dx-iso-base
```

References

fst.bin https://wiki.gbatemp.net/wiki/NKit/Discs#Fst.bin
dol format https://wiibrew.org/wiki/DOL

## Translation build

Build and apply every `.patch` sidecar with:

```sh
nix develop --command cargo build
./target/debug/nasty_dx_dumper patch-all ./tl ./tl-output
```

`patch-all` rewrites every reference to a string, whether a pointer word in a
table or a `lis`/low-half pair in the instruction stream. Whether a
`lis`/low-half pair may be rewritten at all is decided by `data/text-refs.json`.
Those are values I discovered with Ghidra.

Three kinds of string are deliberately not patched yet

- strings addressed off the r2/r13 small-data base, whose +-32K displacement
  cannot reach the arena at all
- strings whose address `text-refs.json` says is used as a value
- fixed-width inline records, which have no reference to redirect and would
  have to be overwritten in place within their glyph budget

It's still TODO to decide what to do with these strings
