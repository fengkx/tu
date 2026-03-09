use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::scanner::{BinaryPolicy, ScanOptions};
use crate::tokenizer::{OpenAiEncoding, TokenizerSpec};

#[derive(Debug, Clone, Parser)]
#[command(name = "tu", version, about = "Count tokens for files and directories")]
pub struct Cli {
    /// Output every file and directory aggregate.
    #[arg(short = 'a', long, conflicts_with = "summarize")]
    pub all: bool,

    /// Output only the summary for each root input.
    #[arg(short = 's', long)]
    pub summarize: bool,

    /// Limit displayed depth. Deeper descendants are still counted in aggregates.
    #[arg(short = 'd', long, value_name = "N")]
    pub max_depth: Option<usize>,

    /// Select the tokenizer backend.
    #[arg(long, value_enum, default_value_t = TokenizerKind::Openai)]
    pub tokenizer: TokenizerKind,

    /// Select the OpenAI encoding.
    #[arg(long, value_enum, default_value_t = OpenAiEncoding::O200kBase)]
    pub encoding: OpenAiEncoding,

    /// Path to a HuggingFace tokenizer.json.
    #[arg(long, value_name = "PATH")]
    pub tokenizer_file: Option<PathBuf>,

    /// Binary file handling policy.
    #[arg(long, value_enum, default_value_t = BinaryPolicy::Skip)]
    pub binary: BinaryPolicy,

    /// Disable .gitignore, .ignore, and git exclude rules.
    #[arg(long)]
    pub no_ignore: bool,

    /// Exclude matching paths. Repeatable.
    #[arg(long, value_name = "GLOB")]
    pub exclude: Vec<String>,

    /// Follow symbolic links.
    #[arg(short = 'L', long)]
    pub follow_links: bool,

    /// Print human-readable token units.
    #[arg(short = 'H', long)]
    pub human: bool,

    /// Emit JSON instead of text output.
    #[arg(long)]
    pub json: bool,

    /// Print a total row when multiple roots are provided.
    #[arg(long)]
    pub total: bool,

    /// Files or directories to scan. Use `-` to read stdin.
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,
}

impl Cli {
    pub fn validate(&self) -> Result<(), String> {
        match self.tokenizer {
            TokenizerKind::Openai => {
                if self.tokenizer_file.is_some() {
                    return Err(String::from(
                        "--tokenizer-file can only be used with --tokenizer hf",
                    ));
                }
            }
            TokenizerKind::Hf => {
                if self.tokenizer_file.is_none() {
                    return Err(String::from(
                        "--tokenizer-file is required when --tokenizer hf is selected",
                    ));
                }
            }
        }

        Ok(())
    }

    pub fn needs_stdin(&self, stdin_is_tty: bool) -> bool {
        self.paths.is_empty() && !stdin_is_tty
            || self
                .paths
                .iter()
                .any(|path| path == std::path::Path::new("-"))
    }

    pub fn tokenizer_spec(&self) -> Result<TokenizerSpec, String> {
        match self.tokenizer {
            TokenizerKind::Openai => Ok(TokenizerSpec::OpenAi {
                encoding: self.encoding,
            }),
            TokenizerKind::Hf => Ok(TokenizerSpec::HuggingFace {
                tokenizer_file: self.tokenizer_file.clone().ok_or_else(|| {
                    String::from("--tokenizer-file is required when --tokenizer hf is selected")
                })?,
            }),
        }
    }
}

impl ScanOptions {
    pub fn from_cli(cli: &Cli) -> Result<Self, String> {
        crate::scanner::validate_excludes(&cli.exclude)?;

        Ok(Self {
            display_all: cli.all,
            max_depth: cli.max_depth,
            binary_policy: cli.binary,
            respect_ignore: !cli.no_ignore,
            follow_links: cli.follow_links,
            exclude: cli.exclude.clone(),
        })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, ValueEnum)]
pub enum TokenizerKind {
    Openai,
    Hf,
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use clap::Parser;

    use super::{Cli, TokenizerKind};
    use crate::tokenizer::{OpenAiEncoding, TokenizerSpec};

    #[test]
    fn validate_rejects_hf_without_tokenizer_file() {
        let cli = Cli::parse_from(["tu", "--tokenizer", "hf", "."]);

        assert_eq!(
            cli.validate(),
            Err(String::from(
                "--tokenizer-file is required when --tokenizer hf is selected",
            )),
        );
    }

    #[test]
    fn validate_rejects_openai_with_tokenizer_file() {
        let cli = Cli::parse_from(["tu", "--tokenizer-file", "fixture.json", "."]);

        assert_eq!(
            cli.validate(),
            Err(String::from(
                "--tokenizer-file can only be used with --tokenizer hf",
            )),
        );
    }

    #[test]
    fn tokenizer_spec_uses_o200k_by_default() {
        let cli = Cli::parse_from(["tu", "."]);

        assert_eq!(
            cli.tokenizer_spec(),
            Ok(TokenizerSpec::OpenAi {
                encoding: OpenAiEncoding::O200kBase,
            }),
        );
    }

    #[test]
    fn tokenizer_spec_builds_hf_variant() {
        let cli = Cli::parse_from([
            "tu",
            "--tokenizer",
            "hf",
            "--tokenizer-file",
            "fixture.json",
            ".",
        ]);

        assert_eq!(
            cli.tokenizer_spec(),
            Ok(TokenizerSpec::HuggingFace {
                tokenizer_file: PathBuf::from("fixture.json"),
            }),
        );
    }

    #[test]
    fn needs_stdin_when_tty_has_explicit_dash() {
        let cli = Cli::parse_from(["tu", "-", "file.txt"]);

        assert!(cli.needs_stdin(true));
    }

    #[test]
    fn needs_stdin_when_pipe_and_no_paths() {
        let cli = Cli::parse_from(["tu"]);

        assert!(cli.needs_stdin(false));
    }

    #[test]
    fn does_not_need_stdin_for_regular_paths() {
        let cli = Cli::parse_from(["tu", "file.txt"]);

        assert!(!cli.needs_stdin(false));
        assert!(!cli.needs_stdin(true));
    }

    #[test]
    fn parse_preserves_selected_tokenizer_kind() {
        let cli = Cli::parse_from([
            "tu",
            "--tokenizer",
            "hf",
            "--tokenizer-file",
            "fixture.json",
            "--encoding",
            "cl100k_base",
            ".",
        ]);

        assert_eq!(cli.tokenizer, TokenizerKind::Hf);
        assert_eq!(cli.encoding, OpenAiEncoding::Cl100kBase);
        assert_eq!(cli.paths, vec![Path::new(".")]);
    }
}
