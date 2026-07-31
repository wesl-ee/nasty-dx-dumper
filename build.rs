use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

fn run(tool: &str, args: &[&str]) {
    let status = Command::new(tool)
        .args(args)
        .status()
        .unwrap_or_else(|e| panic!("failed to run {tool}: {e}"));
    assert!(status.success(), "{tool} failed with {status}");
}

/// Assemble `asm/<stem>.s` down to a raw big-endian instruction blob and assert
/// its length, so a hand-edited source that grows or shrinks fails the build
/// rather than the console.
fn assemble(out: &Path, stem: &str, words: usize) {
    println!("cargo:rerun-if-changed=asm/{stem}.s");

    let object = out.join(format!("{stem}.o"));
    let binary = out.join(format!("{stem}.bin"));
    let object_s = object.to_str().expect("UTF-8 OUT_DIR");
    let binary_s = binary.to_str().expect("UTF-8 OUT_DIR");

    run(
        "powerpc-none-eabi-as",
        &["-mregnames", "-o", object_s, &format!("asm/{stem}.s")],
    );
    run(
        "powerpc-none-eabi-objcopy",
        &["-O", "binary", "--only-section=.text", object_s, binary_s],
    );

    let bytes = std::fs::read(&binary).expect("read assembled PPC patch");
    assert_eq!(bytes.len(), words * 4, "{stem} must be {words} words");
}

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    assemble(&out, "spt_unsigned_offsets", 3);
    // six walkers, ten words each
    assemble(&out, "long_names", 60);
}
