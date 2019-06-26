// Copyright 2015-2017 Parity Technologies (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.	 See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

//! Custom panic hook with bug report link

extern crate backtrace;
extern crate codechain_logger as clogger;
extern crate get_if_addrs;
extern crate my_internet_ip;

use backtrace::Backtrace;
use clogger::EmailAlarm;
use std::panic::{self, PanicInfo};
use std::thread;

/// Set the panic hook
pub fn set() {
    panic::set_hook(Box::new(panic_hook));
}

pub fn set_with_email_alarm(email_alarm: clogger::EmailAlarm) {
    panic::set_hook(Box::new(move |info| panic_hook_with_email_alarm(&email_alarm, info)));
}

static ABOUT_PANIC: &str = "
This is a bug. Please report it at:

    https://github.com/CodeChain-io/codechain/issues/new
";

fn panic_hook(info: &PanicInfo) {
    let message = panic_message(info);
    eprintln!("{}", message);
    exit_on_debug_or_env_set_on_release();
}

fn panic_hook_with_email_alarm(email_alarm: &EmailAlarm, info: &PanicInfo) {
    let message = panic_message(info);
    eprintln!("{}", message);
    let ip_addresses = get_ip_addresses();

    let message_for_email = message.replace("\n", "<br>");
    email_alarm.send(&format!("IP: {}<br>{}", ip_addresses, message_for_email));
    exit_on_debug_or_env_set_on_release();
}

fn panic_message(info: &PanicInfo) -> String {
    let location = info.location();
    let file = location.as_ref().map(|l| l.file()).unwrap_or("<unknown>");
    let line = location.as_ref().map(|l| l.line()).unwrap_or(0);

    let msg = match info.payload().downcast_ref::<&'static str>() {
        Some(s) => *s,
        None => match info.payload().downcast_ref::<String>() {
            Some(s) => &s[..],
            None => "Box<Any>",
        },
    };

    let thread = thread::current();
    let name = thread.name().unwrap_or("<unnamed>");

    let backtrace = Backtrace::new();

    let lines = [
        "".to_string(),
        "====================".to_string(),
        "".to_string(),
        format!("{:?}", backtrace),
        "".to_string(),
        format!("Thread '{}' panicked at '{}', {}:{}", name, msg, file, line),
        ABOUT_PANIC.to_string(),
    ];

    lines.join("\n")
}

#[cfg(debug_assertions)]
fn exit_on_debug_or_env_set_on_release() {
    std::process::exit(-1);
}

#[cfg(not(debug_assertions))]
fn exit_on_debug_or_env_set_on_release() {
    if std::env::var("EXIT_ON_CRASH").is_ok() {
        std::process::exit(-1);
    }
}

fn get_ip_addresses() -> String {
    match my_internet_ip::get() {
        Ok(ip) => return ip.to_string(),
        Err(e) => {
            eprintln!("Failed get internet IP: {:?}", e);
        }
    };

    match get_if_addrs::get_if_addrs() {
        Ok(interfaces) => {
            let ip_addresses: Vec<String> =
                interfaces.iter().map(|interface| format!("{:?}", interface.ip())).collect();
            return ip_addresses.join(", ")
        }
        Err(err) => {
            eprintln!("Failed to get local IPs: {}", err);
        }
    }
    "Unknown".to_string()
}
