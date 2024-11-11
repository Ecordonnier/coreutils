// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// spell-checker:ignore (ToDO) tempdir dyld dylib optgrps libstdbuf

use clap::{Command};
use std::fs::File;
use std::io::Write;
use uucore::error::{UResult};

const STDBUF_INJECT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/libstdbuf.so"));

#[uucore::main]
pub fn uumain(_args: impl uucore::Args) -> UResult<()> {
    let mut file = File::create("foobar.txt")?;
    file.write_all(STDBUF_INJECT)?;
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
}
