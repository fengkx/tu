use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::hf_registry::HfBuiltinTokenizer;
use crate::scanner::{BinaryPolicy, ScanOptions};
use crate::tokenizer::{BuiltinTokenizerId, TokenizerConfig};

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
    #[arg(long, value_enum)]
    pub tokenizer: Option<TokenizerKind>,

    /// Select a builtin tokenizer family. Defaults to `o200k_base`.
    #[arg(long, value_enum)]
    pub encoding: Option<BuiltinTokenizerId>,

    /// Path to a HuggingFace tokenizer.json.
    #[arg(long, value_name = "PATH")]
    pub tokenizer_file: Option<PathBuf>,

    /// Compatibility alias for builtin HuggingFace tokenizer families.
    #[arg(long, value_enum, value_name = "NAME", hide = true)]
    pub hf_tokenizer: Option<HfBuiltinTokenizer>,

    /// Compare multiple tokenizer specs. Repeatable.
    #[arg(long, value_name = "SPEC")]
    pub compare: Vec<String>,

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
        if !self.compare.is_empty() {
            if self.tokenizer.is_some()
                || self.encoding.is_some()
                || self.tokenizer_file.is_some()
                || self.hf_tokenizer.is_some()
            {
                return Err(String::from(
                    "--compare cannot be used with --tokenizer, --encoding, --tokenizer-file, or --hf-tokenizer",
                ));
            }

            self.tokenizer_configs()?;
            return Ok(());
        }

        if self.encoding.is_some() && self.hf_tokenizer.is_some() {
            return Err(String::from(
                "--encoding and --hf-tokenizer cannot be used together",
            ));
        }

        if self.tokenizer_file.is_some() && (self.encoding.is_some() || self.hf_tokenizer.is_some())
        {
            return Err(String::from(
                "--tokenizer-file cannot be used with --encoding or --hf-tokenizer",
            ));
        }

        if let Some(tokenizer) = self.tokenizer {
            match tokenizer {
                TokenizerKind::Openai => {
                    if self.tokenizer_file.is_some() {
                        return Err(String::from(
                            "--tokenizer openai cannot be used with --tokenizer-file",
                        ));
                    }
                }
                TokenizerKind::Hf => {
                    if self.tokenizer_file.is_none()
                        && self.encoding.is_none()
                        && self.hf_tokenizer.is_none()
                    {
                        return Err(String::from(
                            "--tokenizer hf requires --encoding with a HuggingFace builtin or --tokenizer-file",
                        ));
                    }
                }
            }
        }

        if self.tokenizer_file.is_some() {
            return Ok(());
        }

        let builtin = self.builtin_tokenizer_id();
        if let Some(tokenizer) = self.tokenizer {
            if tokenizer == TokenizerKind::Openai && builtin.is_hugging_face() {
                return Err(format!(
                    "--tokenizer openai cannot be used with --encoding {}",
                    builtin.as_str()
                ));
            }

            if tokenizer == TokenizerKind::Hf && !builtin.is_hugging_face() {
                return Err(format!(
                    "--tokenizer hf cannot be used with --encoding {}",
                    builtin.as_str()
                ));
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

    pub fn compare_mode(&self) -> bool {
        !self.compare.is_empty()
    }

    pub fn builtin_tokenizer_id(&self) -> BuiltinTokenizerId {
        self.encoding
            .or_else(|| self.hf_tokenizer.map(BuiltinTokenizerId::from_hf_builtin))
            .unwrap_or(BuiltinTokenizerId::O200kBase)
    }

    pub fn tokenizer_configs(&self) -> Result<Vec<TokenizerConfig>, String> {
        if self.compare_mode() {
            let configs = self
                .compare
                .iter()
                .map(|value| TokenizerConfig::parse_compare_spec(value))
                .collect::<Result<Vec<_>, _>>()?;

            let mut labels = std::collections::BTreeSet::new();
            for config in &configs {
                if !labels.insert(config.label.clone()) {
                    return Err(format!(
                        "duplicate tokenizer label `{}` derived from --compare",
                        config.label
                    ));
                }
            }

            return Ok(configs);
        }

        if let Some(tokenizer_file) = &self.tokenizer_file {
            return Ok(vec![TokenizerConfig::huggingface(tokenizer_file.clone())?]);
        }

        Ok(vec![self.builtin_tokenizer_id().into_tokenizer_config()])
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
    use crate::hf_registry::HfBuiltinTokenizer;
    use crate::tokenizer::{BuiltinTokenizerId, OpenAiEncoding, TokenizerConfig, TokenizerSpec};

    #[test]
    fn validate_rejects_openai_with_tokenizer_file() {
        let cli = Cli::parse_from([
            "tu",
            "--tokenizer",
            "openai",
            "--tokenizer-file",
            "fixture.json",
            ".",
        ]);

        assert_eq!(
            cli.validate(),
            Err(String::from(
                "--tokenizer openai cannot be used with --tokenizer-file",
            )),
        );
    }

    #[test]
    fn validate_rejects_encoding_with_tokenizer_file() {
        let cli = Cli::parse_from([
            "tu",
            "--encoding",
            "qwen3",
            "--tokenizer-file",
            "fixture.json",
            ".",
        ]);

        assert_eq!(
            cli.validate(),
            Err(String::from(
                "--tokenizer-file cannot be used with --encoding or --hf-tokenizer",
            )),
        );
    }

    #[test]
    fn validate_rejects_encoding_with_hf_tokenizer_alias() {
        let cli = Cli::parse_from(["tu", "--encoding", "qwen3", "--hf-tokenizer", "glm5", "."]);

        assert_eq!(
            cli.validate(),
            Err(String::from(
                "--encoding and --hf-tokenizer cannot be used together",
            )),
        );
    }

    #[test]
    fn validate_rejects_hf_with_openai_encoding() {
        let cli = Cli::parse_from(["tu", "--tokenizer", "hf", "--encoding", "o200k_base", "."]);

        assert_eq!(
            cli.validate(),
            Err(String::from(
                "--tokenizer hf cannot be used with --encoding o200k_base",
            )),
        );
    }

    #[test]
    fn validate_rejects_openai_with_hf_encoding() {
        let cli = Cli::parse_from(["tu", "--tokenizer", "openai", "--encoding", "qwen3", "."]);

        assert_eq!(
            cli.validate(),
            Err(String::from(
                "--tokenizer openai cannot be used with --encoding qwen3",
            )),
        );
    }

    #[test]
    fn validate_rejects_hf_without_selector() {
        let cli = Cli::parse_from(["tu", "--tokenizer", "hf", "."]);

        assert_eq!(
            cli.validate(),
            Err(String::from(
                "--tokenizer hf requires --encoding with a HuggingFace builtin or --tokenizer-file",
            )),
        );
    }

    #[test]
    fn tokenizer_configs_uses_o200k_by_default() {
        let cli = Cli::parse_from(["tu", "."]);

        assert_eq!(
            cli.tokenizer_configs(),
            Ok(vec![TokenizerConfig::openai(OpenAiEncoding::O200kBase)]),
        );
    }

    #[test]
    fn tokenizer_configs_build_builtin_from_encoding() {
        let cli = Cli::parse_from(["tu", "--encoding", "qwen3", "."]);

        assert_eq!(
            cli.tokenizer_configs(),
            Ok(vec![BuiltinTokenizerId::Qwen3.into_tokenizer_config()]),
        );
    }

    #[test]
    fn tokenizer_configs_build_builtin_from_hf_alias() {
        let cli = Cli::parse_from(["tu", "--hf-tokenizer", "qwen3", "."]);

        assert_eq!(
            cli.tokenizer_configs(),
            Ok(vec![BuiltinTokenizerId::Qwen3.into_tokenizer_config()]),
        );
    }

    #[test]
    fn tokenizer_configs_build_hf_file_without_explicit_backend() {
        let cli = Cli::parse_from(["tu", "--tokenizer-file", "fixture.json", "."]);

        assert_eq!(
            cli.tokenizer_configs(),
            Ok(vec![
                TokenizerConfig::huggingface(PathBuf::from("fixture.json")).expect("hf config"),
            ]),
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
        let cli = Cli::parse_from(["tu", "--tokenizer", "hf", "--encoding", "qwen3", "."]);

        assert_eq!(cli.tokenizer, Some(TokenizerKind::Hf));
        assert_eq!(cli.encoding, Some(BuiltinTokenizerId::Qwen3));
        assert_eq!(cli.paths, vec![Path::new(".")]);
    }

    #[test]
    fn builtin_tokenizer_id_defaults_to_o200k() {
        let cli = Cli::parse_from(["tu", "."]);

        assert_eq!(cli.builtin_tokenizer_id(), BuiltinTokenizerId::O200kBase);
    }

    #[test]
    fn builtin_tokenizer_id_uses_hf_alias_when_present() {
        let cli = Cli::parse_from(["tu", "--hf-tokenizer", "glm5", "."]);

        assert_eq!(cli.builtin_tokenizer_id(), BuiltinTokenizerId::Glm5);
    }

    #[test]
    fn validate_rejects_compare_with_single_tokenizer_flags() {
        let cli = Cli::parse_from([
            "tu",
            "--compare",
            "o200k_base",
            "--encoding",
            "cl100k_base",
            ".",
        ]);

        assert_eq!(
            cli.validate(),
            Err(String::from(
                "--compare cannot be used with --tokenizer, --encoding, --tokenizer-file, or --hf-tokenizer",
            )),
        );
    }

    #[test]
    fn tokenizer_configs_build_compare_list_in_order() {
        let cli = Cli::parse_from(["tu", "--compare", "o200k_base", "--compare", "qwen3", "."]);

        assert_eq!(
            cli.tokenizer_configs(),
            Ok(vec![
                TokenizerConfig::openai(OpenAiEncoding::O200kBase),
                BuiltinTokenizerId::Qwen3.into_tokenizer_config(),
            ]),
        );
    }

    #[test]
    fn tokenizer_configs_reject_duplicate_compare_labels() {
        let cli = Cli::parse_from([
            "tu",
            "--compare",
            "file:foo/tokenizer.json",
            "--compare",
            "hf:bar/tokenizer.json",
            ".",
        ]);

        assert_eq!(
            cli.tokenizer_configs(),
            Err(String::from(
                "duplicate tokenizer label `hf:tokenizer.json` derived from --compare",
            )),
        );
    }

    #[test]
    fn compatibility_alias_keeps_hf_builtin_shape() {
        let cli = Cli::parse_from(["tu", "--hf-tokenizer", "qwen3", "."]);
        let config = cli
            .tokenizer_configs()
            .expect("tokenizer config")
            .into_iter()
            .next()
            .expect("one config");

        assert_eq!(
            config.spec,
            TokenizerSpec::hf_builtin(HfBuiltinTokenizer::Qwen3)
        );
    }
}
