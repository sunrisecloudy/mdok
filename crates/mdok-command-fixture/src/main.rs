//! Deterministic external-process fixture used by stored MDOK command tests.

#![forbid(unsafe_code)]

use std::env;
use std::io::{self, Write};
use std::process::ExitCode;
use std::thread;
use std::time::Duration;

fn main() -> ExitCode {
    let mut arguments = env::args().skip(1);
    let Some(command) = arguments.next() else {
        eprintln!("missing fixture command");
        return ExitCode::from(2);
    };
    match command.as_str() {
        "json" => {
            println!(r#"{{"ok":true,"source":"mdok-command-fixture"}}"#);
            ExitCode::SUCCESS
        }
        "echo" => {
            let mut stdout = io::BufWriter::new(io::stdout().lock());
            for argument in arguments {
                writeln!(stdout, "{argument}").expect("write fixture output");
            }
            ExitCode::SUCCESS
        }
        "env" => {
            let Some(name) = arguments.next() else {
                eprintln!("missing environment variable name");
                return ExitCode::from(2);
            };
            match env::var_os(name) {
                Some(value) => println!("{}", value.to_string_lossy()),
                None => println!("<unset>"),
            }
            ExitCode::SUCCESS
        }
        "sleep-ms" => {
            let Some(value) = arguments.next() else {
                eprintln!("missing sleep duration");
                return ExitCode::from(2);
            };
            let Ok(milliseconds) = value.parse::<u64>() else {
                eprintln!("invalid sleep duration");
                return ExitCode::from(2);
            };
            thread::sleep(Duration::from_millis(milliseconds));
            ExitCode::SUCCESS
        }
        "spam" => {
            let Some(value) = arguments.next() else {
                eprintln!("missing byte count");
                return ExitCode::from(2);
            };
            let Ok(count) = value.parse::<usize>() else {
                eprintln!("invalid byte count");
                return ExitCode::from(2);
            };
            let mut stdout = io::BufWriter::new(io::stdout().lock());
            let chunk = [b'x'; 4096];
            let mut remaining = count;
            while remaining > 0 {
                let length = remaining.min(chunk.len());
                stdout
                    .write_all(&chunk[..length])
                    .expect("write fixture output");
                remaining -= length;
            }
            ExitCode::SUCCESS
        }
        "nonzero" => {
            eprintln!("fixture failure");
            ExitCode::from(7)
        }
        _ => {
            eprintln!("unknown fixture command: {command}");
            ExitCode::from(2)
        }
    }
}
