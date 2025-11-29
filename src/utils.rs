pub fn print_hex_dump(data: &[u8], max_bytes: usize) {
    let bytes_to_show = std::cmp::min(data.len(), max_bytes);
    for (i, byte) in data[..bytes_to_show].iter().enumerate() {
        if i % 16 == 0 && i > 0 {
            println!();
        }
        if i % 16 == 0 {
            print!("{:08x}: ", i);
        }
        print!("{:02x} ", byte);
    }
    if bytes_to_show > 0 {
        println!();
    }
}
