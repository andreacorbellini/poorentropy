// Copyright © 2024 Andrea Corbellini and contributors
// SPDX-License-Identifier: BSD-2-Clause

use clap::arg;
use clap::value_parser;
use clap::Command;
use std::io;
use std::io::ErrorKind::BrokenPipe;

fn main() -> io::Result<()> {
    let args = cli().get_matches();
    let out = io::stdout().lock();
    if let Some(count) = args.get_one("count") {
        print_bytes(out, poorentropy::bytes().take(*count))
    } else {
        print_bytes(out, poorentropy::bytes())
    }
}

fn cli() -> Command {
    Command::new("testgen")
        .about("Prints random bytes generated by poorentropy")
        .arg(
            arg!(-c --count <N> "quit after printing the given number of bytes")
                .value_parser(value_parser!(usize)),
        )
}

fn print_bytes<W, I>(mut out: W, iter: I) -> io::Result<()>
where
    W: io::Write,
    I: IntoIterator<Item = u8>,
{
    for byte in iter {
        match out.write_all(&[byte]) {
            Ok(()) => (),
            Err(e) if e.kind() == BrokenPipe => break,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
