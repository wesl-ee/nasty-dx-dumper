use anyhow::{Context, Result};
use std::process;

use crate::text::{create_reverse_character_table, encode_text_to_bytes};

pub fn encode(text: String) -> Result<()> {
    println!("Encoding text: \"{}\"", text);

    let char_map = create_reverse_character_table()
        .with_context(|| "Failed to create reverse character table")?;

    match encode_text_to_bytes(&text, &char_map) {
        Ok(bytes) => {
            println!("Encoded bytes ({} bytes):", bytes.len());

            for (i, byte) in bytes.iter().enumerate() {
                if i % 16 == 0 && i > 0 {
                    println!();
                }
                if i % 16 == 0 {
                    print!("{:08x}: ", i);
                }
                print!("{:02x} ", byte);
            }
            if !bytes.is_empty() {
                println!();
            }

            println!("\nAs 16-bit BE values:");
            for chunk in bytes.chunks(2) {
                if chunk.len() == 2 {
                    let value = u16::from_be_bytes([chunk[0], chunk[1]]);
                    print!("0x{:04X} ", value);
                }
            }
            if !bytes.is_empty() {
                println!();
            }
        }
        Err(e) => {
            eprintln!("Error encoding text: {}", e);
            process::exit(1);
        }
    }
    Ok(())
}
