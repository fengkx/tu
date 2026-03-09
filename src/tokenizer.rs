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

pub enum TokenizerBackend {
    OpenAi(CoreBPE),
    HuggingFace(Tokenizer),
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

    use super::{OpenAiEncoding, TokenizerBackend, TokenizerSpec};

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
}
