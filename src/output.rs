use serde::Serialize;

use crate::cli::Cli;
use crate::scanner::{EntryStat, RootScanResult};
use crate::tokenizer::TokenizerSpec;

pub fn render_text(cli: &Cli, results: &[RootScanResult]) -> String {
    let mut lines = Vec::new();

    for result in results {
        let entries = if cli.all {
            &result.entries
        } else {
            std::slice::from_ref(&result.root)
        };

        lines.extend(entries.iter().map(|entry| format_entry(entry, cli.human)));
    }

    if cli.total && results.len() > 1 {
        let total = sum_entries(results.iter().map(|result| &result.root));
        lines.push(format_entry(&total, cli.human));
    }

    let mut output = lines.join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    output
}

pub fn render_json(
    tokenizer: &TokenizerSpec,
    results: &[RootScanResult],
) -> Result<String, String> {
    let payload = JsonOutput {
        tokenizer,
        entries: results
            .iter()
            .flat_map(|result| result.entries.iter().cloned())
            .collect(),
        total: sum_entries(results.iter().map(|result| &result.root)),
        had_errors: results.iter().any(RootScanResult::had_errors),
    };

    serde_json::to_string_pretty(&payload).map_err(|err| err.to_string())
}

fn format_entry(entry: &EntryStat, human: bool) -> String {
    let tokens = if human {
        humanize_tokens(entry.tokens)
    } else {
        entry.tokens.to_string()
    };
    format!("{tokens}\t{}", entry.path)
}

fn sum_entries<'a>(entries: impl Iterator<Item = &'a EntryStat>) -> EntryStat {
    let mut total = EntryStat {
        path: String::from("total"),
        kind: crate::scanner::EntryKind::Dir,
        tokens: 0,
        files: 0,
        skipped: 0,
        errors: 0,
        depth: 0,
    };

    for entry in entries {
        total.tokens += entry.tokens;
        total.files += entry.files;
        total.skipped += entry.skipped;
        total.errors += entry.errors;
    }

    total
}

fn humanize_tokens(tokens: u64) -> String {
    const UNITS: [&str; 5] = ["", "K", "M", "B", "T"];

    let mut value = tokens as f64;
    let mut unit = 0usize;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }

    if unit == 0 {
        return tokens.to_string();
    }

    if value >= 10.0 {
        format!("{value:.0}{}", UNITS[unit])
    } else {
        format!("{value:.1}{}", UNITS[unit])
    }
}

#[derive(Debug, Serialize)]
struct JsonOutput<'a> {
    tokenizer: &'a TokenizerSpec,
    entries: Vec<EntryStat>,
    total: EntryStat,
    had_errors: bool,
}

#[cfg(test)]
mod tests {
    use super::{humanize_tokens, render_json, render_text, sum_entries};
    use crate::cli::Cli;
    use crate::scanner::{Diagnostic, DiagnosticLevel, EntryKind, EntryStat, RootScanResult};
    use crate::tokenizer::{OpenAiEncoding, TokenizerSpec};
    use clap::Parser;
    use serde_json::Value;

    fn file_entry(path: &str, tokens: u64) -> EntryStat {
        EntryStat {
            path: path.to_string(),
            kind: EntryKind::File,
            tokens,
            files: 1,
            skipped: 0,
            errors: 0,
            depth: 1,
        }
    }

    fn dir_entry(path: &str, tokens: u64, files: u64) -> EntryStat {
        EntryStat {
            path: path.to_string(),
            kind: EntryKind::Dir,
            tokens,
            files,
            skipped: 0,
            errors: 0,
            depth: 0,
        }
    }

    #[test]
    fn humanize_tokens_handles_boundaries() {
        assert_eq!(humanize_tokens(999), "999");
        assert_eq!(humanize_tokens(1_000), "1.0K");
        assert_eq!(humanize_tokens(1_500), "1.5K");
        assert_eq!(humanize_tokens(10_000), "10K");
        assert_eq!(humanize_tokens(1_000_000), "1.0M");
    }

    #[test]
    fn sum_entries_accumulates_counts() {
        let total = sum_entries(
            [
                &EntryStat {
                    path: String::from("a"),
                    kind: EntryKind::File,
                    tokens: 2,
                    files: 1,
                    skipped: 1,
                    errors: 0,
                    depth: 0,
                },
                &EntryStat {
                    path: String::from("b"),
                    kind: EntryKind::File,
                    tokens: 3,
                    files: 1,
                    skipped: 0,
                    errors: 1,
                    depth: 0,
                },
            ]
            .into_iter(),
        );

        assert_eq!(total.path, "total");
        assert_eq!(total.tokens, 5);
        assert_eq!(total.files, 2);
        assert_eq!(total.skipped, 1);
        assert_eq!(total.errors, 1);
    }

    #[test]
    fn render_text_supports_human_and_total_rows() {
        let cli = Cli::parse_from(["tu", "--human", "--total", "first", "second"]);
        let results = vec![
            RootScanResult {
                root: dir_entry("first", 1_500, 1),
                entries: vec![file_entry("first/file.txt", 1_500)],
                diagnostics: Vec::new(),
            },
            RootScanResult {
                root: dir_entry("second", 600, 1),
                entries: vec![file_entry("second/file.txt", 600)],
                diagnostics: Vec::new(),
            },
        ];

        let output = render_text(&cli, &results);

        assert!(output.contains("1.5K\tfirst"));
        assert!(output.contains("600\tsecond"));
        assert!(output.contains("2.1K\ttotal"));
    }

    #[test]
    fn render_json_includes_total_and_had_errors() {
        let results = vec![RootScanResult {
            root: dir_entry("root", 2, 1),
            entries: vec![file_entry("root/file.txt", 2)],
            diagnostics: vec![Diagnostic {
                level: DiagnosticLevel::Error,
                message: String::from("root/file.txt: binary input encountered"),
            }],
        }];

        let rendered = render_json(
            &TokenizerSpec::OpenAi {
                encoding: OpenAiEncoding::O200kBase,
            },
            &results,
        )
        .expect("json");
        let parsed: Value = serde_json::from_str(&rendered).expect("parse json");

        assert_eq!(parsed["tokenizer"]["kind"], "open_ai");
        assert_eq!(parsed["total"]["tokens"], 2);
        assert_eq!(parsed["entries"][0]["path"], "root/file.txt");
        assert_eq!(parsed["had_errors"], false);
    }
}
