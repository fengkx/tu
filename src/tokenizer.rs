use std::path::PathBuf;

use clap::ValueEnum;
use serde::Serialize;
use tiktoken_rs::{CoreBPE, cl100k_base, o200k_base, p50k_base, r50k_base};
use tokenizers::Tokenizer;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum OpenAiEncoding {
    #[value(name = "o200k_base")]
    O200kBase,
    #[value(name = "cl100k_base")]
    Cl100kBase,
    #[value(name = "p50k_base")]
    P50kBase,
    #[value(name = "r50k_base")]
    R50kBase,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TokenizerSpec {
    OpenAi { encoding: OpenAiEncoding },
    HuggingFace { tokenizer_file: PathBuf },
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct TokenizerConfig {
    pub label: String,
    #[serde(flatten)]
    pub spec: TokenizerSpec,
}

pub enum TokenizerBackend {
    OpenAi(CoreBPE),
    HuggingFace(Tokenizer),
}

impl OpenAiEncoding {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "o200k_base" => Ok(Self::O200kBase),
            "cl100k_base" => Ok(Self::Cl100kBase),
            "p50k_base" => Ok(Self::P50kBase),
            "r50k_base" => Ok(Self::R50kBase),
            _ => Err(format!("unsupported OpenAI encoding `{value}`")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::O200kBase => "o200k_base",
            Self::Cl100kBase => "cl100k_base",
            Self::P50kBase => "p50k_base",
            Self::R50kBase => "r50k_base",
        }
    }
}

impl TokenizerConfig {
    pub fn openai(encoding: OpenAiEncoding) -> Self {
        Self {
            label: encoding.as_str().to_string(),
            spec: TokenizerSpec::OpenAi { encoding },
        }
    }

    pub fn huggingface(tokenizer_file: PathBuf) -> Result<Self, String> {
        let file_name = tokenizer_file
            .file_name()
            .ok_or_else(|| {
                format!(
                    "failed to derive HuggingFace tokenizer label from `{}`",
                    tokenizer_file.display()
                )
            })?
            .to_string_lossy();

        Ok(Self {
            label: format!("hf:{file_name}"),
            spec: TokenizerSpec::HuggingFace { tokenizer_file },
        })
    }

    pub fn parse_compare_spec(value: &str) -> Result<Self, String> {
        let (kind, raw_value) = value
            .split_once(':')
            .ok_or_else(|| format!("invalid tokenizer spec `{value}`"))?;

        if raw_value.is_empty() {
            return Err(format!("invalid tokenizer spec `{value}`"));
        }

        match kind {
            "openai" => OpenAiEncoding::parse(raw_value).map(Self::openai),
            "hf" => Self::huggingface(PathBuf::from(raw_value)),
            _ => Err(format!("unsupported tokenizer kind `{kind}`")),
        }
    }
}

impl TokenizerBackend {
    pub fn from_spec(spec: &TokenizerSpec) -> Result<Self, String> {
        match spec {
            TokenizerSpec::OpenAi { encoding } => {
                let bpe = match encoding {
                    OpenAiEncoding::O200kBase => o200k_base(),
                    OpenAiEncoding::Cl100kBase => cl100k_base(),
                    OpenAiEncoding::P50kBase => p50k_base(),
                    OpenAiEncoding::R50kBase => r50k_base(),
                }
                .map_err(|err| err.to_string())?;

                Ok(Self::OpenAi(bpe))
            }
            TokenizerSpec::HuggingFace { tokenizer_file } => Tokenizer::from_file(tokenizer_file)
                .map(Self::HuggingFace)
                .map_err(|err| err.to_string()),
        }
    }

    pub fn count(&mut self, text: &str) -> Result<u64, String> {
        match self {
            Self::OpenAi(bpe) => Ok(bpe.encode_ordinary(text).len() as u64),
            Self::HuggingFace(tokenizer) => tokenizer
                .encode(text, false)
                .map(|encoding| encoding.len() as u64)
                .map_err(|err| err.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{OpenAiEncoding, TokenizerBackend, TokenizerConfig, TokenizerSpec};

    fn hf_fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("hf-tokenizer.json")
    }

    #[test]
    fn openai_o200k_base_count_is_stable() {
        let mut backend = TokenizerBackend::from_spec(&TokenizerSpec::OpenAi {
            encoding: OpenAiEncoding::O200kBase,
        })
        .expect("openai tokenizer");

        let count = backend.count("hello world").expect("count");

        assert_eq!(count, 2);
    }

    #[test]
    fn huggingface_fixture_loads_and_counts() {
        let mut backend = TokenizerBackend::from_spec(&TokenizerSpec::HuggingFace {
            tokenizer_file: hf_fixture(),
        })
        .expect("hf tokenizer");

        let count = backend.count("hello world").expect("count");

        assert_eq!(count, 1);
    }

    #[test]
    fn parse_openai_compare_spec_uses_encoding_name_as_label() {
        let config = TokenizerConfig::parse_compare_spec("openai:o200k_base").expect("config");

        assert_eq!(config.label, "o200k_base");
        assert_eq!(
            config.spec,
            TokenizerSpec::OpenAi {
                encoding: OpenAiEncoding::O200kBase,
            }
        );
    }

    #[test]
    fn parse_hf_compare_spec_uses_file_name_as_label() {
        let config =
            TokenizerConfig::parse_compare_spec("hf:tests/fixtures/hf-tokenizer.json")
                .expect("config");

        assert_eq!(config.label, "hf:hf-tokenizer.json");
        assert_eq!(
            config.spec,
            TokenizerSpec::HuggingFace {
                tokenizer_file: PathBuf::from("tests/fixtures/hf-tokenizer.json"),
            }
        );
    }

    #[test]
    fn parse_compare_spec_rejects_unknown_kind() {
        assert_eq!(
            TokenizerConfig::parse_compare_spec("custom:foo"),
            Err(String::from("unsupported tokenizer kind `custom`")),
        );
    }
}
