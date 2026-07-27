use std::{env, path::PathBuf, process::Command};

fn run(tool: &str, args: &[&str]) {
    let status = Command::new(tool)
        .args(args)
        .status()
        .unwrap_or_else(|e| panic!("failed to run {tool}: {e}"));
    assert!(status.success(), "{tool} failed with {status}");
}

fn main() {
    println!("cargo:rerun-if-changed=asm/spt_unsigned_offsets.s");

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let object = out.join("spt_unsigned_offsets.o");
    let binary = out.join("spt_unsigned_offsets.bin");
    let object_s = object.to_str().expect("UTF-8 OUT_DIR");
    let binary_s = binary.to_str().expect("UTF-8 OUT_DIR");

    run(
        "powerpc-none-eabi-as",
        &["-mregnames", "-o", object_s, "asm/spt_unsigned_offsets.s"],
    );
    run(
        "powerpc-none-eabi-objcopy",
        &["-O", "binary", "--only-section=.text", object_s, binary_s],
    );

    let bytes = std::fs::read(&binary).expect("read assembled PPC patch");
    assert_eq!(bytes.len(), 12, "SPT resolver patch must be three words");
}
