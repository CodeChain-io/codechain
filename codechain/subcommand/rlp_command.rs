// Copyright 2019 Kodebox, Inc.
// This file is part of CodeChain.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::io::stdin;

use clap::ArgMatches;
use rustc_hex::{FromHex, ToHex};

use primitives::remove_0x_prefix;
use rlp::{DecoderError, UntrustedRlp};

pub fn run_rlp_command(matches: &ArgMatches) -> Result<(), String> {
    let mut buffer = String::new();
    let rlp_string = if let Some(rlp) = matches.value_of("rlp") {
        remove_0x_prefix(rlp)
    } else {
        stdin().read_line(&mut buffer).map_err(|e| e.to_string())?;
        remove_0x_prefix(&buffer)
    };
    let byte_array = rlp_string.from_hex().map_err(|e| e.to_string())?;
    visualize(&UntrustedRlp::new(&byte_array), 0).map_err(|e| format!("Error while decoding: {}", e))?;
    println!();
    Ok(())
}

fn visualize(rlp: &UntrustedRlp, depth: usize) -> Result<(), DecoderError> {
    if rlp.is_list() {
        println!("[");
        for rlp in rlp.iter() {
            indent(depth + 1);
            visualize(&rlp, depth + 1)?;
            println!(",");
        }
        indent(depth);
        print!("]");
    } else if rlp.is_null() {
        print!(r#"null"#);
    } else if rlp.is_data() {
        let data = rlp.data()?;
        if let Ok(int) = rlp.as_val::<u64>() {
            print!("0x{} ({})", data.to_hex(), int);
            return Ok(())
        }
        print!("#{:2}:0x{}", data.len(), data.to_hex());
        if let Ok(text) = std::str::from_utf8(data) {
            println!();
            indent(depth);
            if text.is_ascii() {
                print!(r#"(ascii: "{}")"#, text);
            } else {
                print!(r#"(utf-8: "{}")"#, text);
            }
        } else if let Ok(payload) = UntrustedRlp::new(data).payload_info() {
            if payload.total() == data.len() {
                println!();
                indent(depth);
                print!("(rlp: ");
                visualize(&UntrustedRlp::new(data), depth)?;
                print!(")");
            }
        }
    }
    Ok(())
}

fn indent(depth: usize) {
    for _ in 0..depth * 4 {
        print!(" ");
    }
}
