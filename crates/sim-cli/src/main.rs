use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let mut args = env::args().skip(1);
    let Some(netlist_path) = args.next() else {
        eprintln!("usage: sim-cli <netlist>");
        std::process::exit(2);
    };

    let path = Path::new(&netlist_path);
    if !path.exists() {
        eprintln!("netlist not found: {}", netlist_path);
        std::process::exit(2);
    }

    let content = fs::read_to_string(path).unwrap_or_default();
    let mut has_end = false;
    let mut commands = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('*') {
            continue;
        }
        if line.starts_with('+') {
            continue;
        }
        if line.starts_with('.') {
            let cmd = line.split_whitespace().next().unwrap_or("");
            commands.push(cmd.to_ascii_lowercase());
            if cmd.eq_ignore_ascii_case(".end") {
                has_end = true;
            }
        }
    }

    if !has_end {
        eprintln!("netlist missing .end: {}", netlist_path);
        std::process::exit(2);
    }

    println!(
        "parsed netlist: {} commands={}",
        netlist_path,
        commands.len()
    );
}
