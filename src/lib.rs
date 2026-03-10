mod cli;
mod output;
mod scanner;
mod tokenizer;

use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::thread;

use clap::Parser;

use cli::Cli;
use output::{render_json, render_text};
use scanner::{RootScanResult, ScanOptions, ScanRoot, scan_root};
use tokenizer::{TokenizerBackend, TokenizerConfig};

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

    let compare_mode = cli.compare_mode();
    let tokenizer_configs = cli.tokenizer_configs()?;
    let scan_options = ScanOptions::from_cli(&cli)?;
    let roots = build_roots(&cli, stdin_is_tty);
    let stdin = stdin_bytes.unwrap_or_default().to_vec();
    let runs = run_tokenizers(&cli, &tokenizer_configs, &scan_options, &roots, &stdin)?;

    let exit_code = if runs.iter().any(TokenizerRunResult::had_errors) {
        1
    } else {
        0
    };

    let stderr = runs
        .iter()
        .flat_map(|run| {
            run.results.iter().flat_map(move |result| {
                result.diagnostics.iter().map(move |diag| {
                    if compare_mode {
                        format!(
                            "{}: [{}] {}\n",
                            diag.level.label(),
                            run.tokenizer.label,
                            diag.message
                        )
                    } else {
                        format!("{}: {}\n", diag.level.label(), diag.message)
                    }
                })
            })
        })
        .collect::<String>();

    let stdout = if cli.json {
        render_json(&cli, &runs)?
    } else {
        render_text(&cli, &runs)?
    };

    Ok(Execution {
        stdout,
        stderr,
        exit_code,
    })
}

fn run_tokenizers(
    cli: &Cli,
    tokenizer_configs: &[TokenizerConfig],
    scan_options: &ScanOptions,
    roots: &[ScanRoot],
    stdin_bytes: &[u8],
) -> Result<Vec<TokenizerRunResult>, String> {
    if !cli.compare_mode() {
        return tokenizer_configs
            .first()
            .cloned()
            .map(|config| run_tokenizer(config, scan_options.clone(), roots.to_vec(), stdin_bytes.to_vec()))
            .transpose()?
            .map(|run| vec![run])
            .ok_or_else(|| String::from("no tokenizer configuration resolved"));
    }

    let handles = tokenizer_configs
        .iter()
        .cloned()
        .map(|config| {
            let scan_options = scan_options.clone();
            let roots = roots.to_vec();
            let stdin_bytes = stdin_bytes.to_vec();
            thread::spawn(move || run_tokenizer(config, scan_options, roots, stdin_bytes))
        })
        .collect::<Vec<_>>();

    handles
        .into_iter()
        .map(|handle| {
            handle
                .join()
                .map_err(|_| String::from("tokenizer worker thread panicked"))?
        })
        .collect()
}

fn run_tokenizer(
    tokenizer: TokenizerConfig,
    scan_options: ScanOptions,
    roots: Vec<ScanRoot>,
    stdin_bytes: Vec<u8>,
) -> Result<TokenizerRunResult, String> {
    let mut backend = TokenizerBackend::from_spec(&tokenizer.spec)?;
    let mut results = Vec::with_capacity(roots.len());

    for root in roots {
        results.push(scan_root(&root, &scan_options, &mut backend, &stdin_bytes));
    }

    Ok(TokenizerRunResult { tokenizer, results })
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

#[derive(Debug, Clone)]
pub struct TokenizerRunResult {
    pub tokenizer: TokenizerConfig,
    pub results: Vec<RootScanResult>,
}

impl TokenizerRunResult {
    pub fn had_errors(&self) -> bool {
        self.results.iter().any(RootScanResult::had_errors)
    }
}
