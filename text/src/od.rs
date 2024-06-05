use std::env;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

fn print_hex_dump<R: Read>(reader: &mut R) -> io::Result<()> {
    let mut buffer = [0; 16];
    let mut offset = 0;

    while let Ok(n) = reader.read(&mut buffer) {
        if n == 0 {
            break;
        }

        print!("{:08x}  ", offset);
        for i in 0..16 {
            if i < n {
                print!("{:02x} ", buffer[i]);
            } else {
                print!("   ");
            }

            if i == 7 {
                print!(" ");
            }
        }

        print!(" |");

        for i in 0..n {
            let c = buffer[i];
            if c.is_ascii_graphic() || c == b' ' {
                print!("{}", c as char);
            } else {
                print!(".");
            }
        }

        println!("|");

        offset += n;
    }

    Ok(())
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <file>", args[0]);
        return Ok(());
    }

    let path = Path::new(&args[1]);
    let mut file = File::open(&path)?;

    print_hex_dump(&mut file)
}
