mod cli;
mod output;
mod scanner;
mod tokenizer;

use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};

use clap::Parser;

use cli::Cli;
use output::{render_json, render_text};
use scanner::{RootScanResult, ScanOptions, ScanRoot, scan_root};
use tokenizer::TokenizerBackend;

pub fn run() -> i32 {
    let cli = Cli::parse();
    let stdin_is_tty = io::stdin().is_terminal();

    let needs_stdin = cli.needs_stdin(stdin_is_tty);
    let stdin_bytes = match read_stdin(needs_stdin) {
        Ok(stdin) => stdin,
        Err(err) => {
            eprintln!("error: failed to read stdin: {err}");
            return 1;
        }
    };

    match execute(cli, stdin_is_tty, stdin_bytes.as_deref()) {
        Ok(execution) => {
            print!("{}", execution.stdout);
            eprint!("{}", execution.stderr);
            execution.exit_code
        }
        Err(err) => {
            eprintln!("error: {err}");
            2
        }
    }
}

fn read_stdin(needs_stdin: bool) -> io::Result<Option<Vec<u8>>> {
    if !needs_stdin {
        return Ok(None);
    }

    let mut buffer = Vec::new();
    io::stdin().read_to_end(&mut buffer)?;
    Ok(Some(buffer))
}

fn execute(cli: Cli, stdin_is_tty: bool, stdin_bytes: Option<&[u8]>) -> Result<Execution, String> {
    cli.validate()?;

    let tokenizer_spec = cli.tokenizer_spec()?;
    let mut tokenizer = TokenizerBackend::from_spec(&tokenizer_spec)?;
    let scan_options = ScanOptions::from_cli(&cli)?;
    let roots = build_roots(&cli, stdin_is_tty);

    let mut root_results = Vec::with_capacity(roots.len());
    for root in roots {
        let stdin = stdin_bytes.unwrap_or_default();
        root_results.push(scan_root(&root, &scan_options, &mut tokenizer, stdin));
    }

    let exit_code = if root_results.iter().any(RootScanResult::had_errors) {
        1
    } else {
        0
    };

    let stderr = root_results
        .iter()
        .flat_map(|result| result.diagnostics.iter())
        .map(|diag| format!("{}: {}\n", diag.level.label(), diag.message))
        .collect::<String>();

    let stdout = if cli.json {
        render_json(&tokenizer_spec, &root_results)?
    } else {
        render_text(&cli, &root_results)
    };

    Ok(Execution {
        stdout,
        stderr,
        exit_code,
    })
}

fn build_roots(cli: &Cli, stdin_is_tty: bool) -> Vec<ScanRoot> {
    if cli.paths.is_empty() {
        if stdin_is_tty {
            return vec![ScanRoot::Path(PathBuf::from("."))];
        }
        return vec![ScanRoot::Stdin];
    }

    cli.paths
        .iter()
        .map(|path| {
            if path == Path::new("-") {
                ScanRoot::Stdin
            } else {
                ScanRoot::Path(path.clone())
            }
        })
        .collect()
}

struct Execution {
    stdout: String,
    stderr: String,
    exit_code: i32,
}
