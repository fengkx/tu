use std::path::PathBuf;

use clap::ValueEnum;
use serde::Serialize;
use tiktoken_rs::{CoreBPE, cl100k_base, o200k_base, p50k_base, r50k_base};
use tokenizers::Tokenizer;

use crate::hf_registry::HfBuiltinTokenizer;

#[derive(Debug, Clone, Copy, Eq, PartialEq, ValueEnum)]
pub enum BuiltinTokenizerId {
    #[value(name = "o200k_base")]
    O200kBase,
    #[value(name = "cl100k_base")]
    Cl100kBase,
    #[value(name = "p50k_base")]
    P50kBase,
    #[value(name = "r50k_base")]
    R50kBase,
    #[value(name = "qwen3")]
    Qwen3,
    #[value(name = "deepseek_v3_2")]
    DeepseekV32,
    #[value(name = "glm5")]
    Glm5,
}

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
    OpenAi {
        encoding: OpenAiEncoding,
    },
    HuggingFace {
        #[serde(flatten)]
        spec: HuggingFaceTokenizerSpec,
    },
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct HuggingFaceTokenizerSpec {
    pub source: HuggingFaceTokenizerSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<HfBuiltinTokenizer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenizer_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HuggingFaceTokenizerSource {
    Builtin,
    File,
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

impl BuiltinTokenizerId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::O200kBase => "o200k_base",
            Self::Cl100kBase => "cl100k_base",
            Self::P50kBase => "p50k_base",
            Self::R50kBase => "r50k_base",
            Self::Qwen3 => "qwen3",
            Self::DeepseekV32 => "deepseek_v3_2",
            Self::Glm5 => "glm5",
        }
    }

    pub fn is_hugging_face(self) -> bool {
        matches!(self, Self::Qwen3 | Self::DeepseekV32 | Self::Glm5)
    }

    pub fn from_hf_builtin(name: HfBuiltinTokenizer) -> Self {
        match name {
            HfBuiltinTokenizer::Qwen3 => Self::Qwen3,
            HfBuiltinTokenizer::DeepseekV32 => Self::DeepseekV32,
            HfBuiltinTokenizer::Glm5 => Self::Glm5,
        }
    }

    pub fn into_tokenizer_config(self) -> TokenizerConfig {
        match self {
            Self::O200kBase => TokenizerConfig::openai(OpenAiEncoding::O200kBase),
            Self::Cl100kBase => TokenizerConfig::openai(OpenAiEncoding::Cl100kBase),
            Self::P50kBase => TokenizerConfig::openai(OpenAiEncoding::P50kBase),
            Self::R50kBase => TokenizerConfig::openai(OpenAiEncoding::R50kBase),
            Self::Qwen3 => TokenizerConfig::huggingface_builtin(HfBuiltinTokenizer::Qwen3),
            Self::DeepseekV32 => {
                TokenizerConfig::huggingface_builtin(HfBuiltinTokenizer::DeepseekV32)
            }
            Self::Glm5 => TokenizerConfig::huggingface_builtin(HfBuiltinTokenizer::Glm5),
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
            spec: TokenizerSpec::hf_file(tokenizer_file),
        })
    }

    pub fn huggingface_builtin(name: HfBuiltinTokenizer) -> Self {
        Self {
            label: format!("hf:{}", name.as_str()),
            spec: TokenizerSpec::hf_builtin(name),
        }
    }

    pub fn parse_compare_spec(value: &str) -> Result<Self, String> {
        if let Ok(id) = BuiltinTokenizerId::from_str(value, false) {
            return Ok(id.into_tokenizer_config());
        }

        let (kind, raw_value) = value
            .split_once(':')
            .ok_or_else(|| format!("invalid tokenizer spec `{value}`"))?;

        if raw_value.is_empty() {
            return Err(format!("invalid tokenizer spec `{value}`"));
        }

        match kind {
            "file" => Self::huggingface(PathBuf::from(raw_value)),
            "openai" => OpenAiEncoding::parse(raw_value).map(Self::openai),
            "hf_builtin" => HfBuiltinTokenizer::from_str(raw_value, false)
                .map(Self::huggingface_builtin)
                .map_err(|_| format!("unsupported HuggingFace builtin tokenizer `{raw_value}`")),
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
            TokenizerSpec::HuggingFace { spec } => match spec.source {
                HuggingFaceTokenizerSource::Builtin => {
                    let name = spec.name.ok_or_else(|| {
                        String::from("missing builtin tokenizer name for hugging face backend")
                    })?;

                    Tokenizer::from_bytes(name.load_bytes()?)
                        .map(Self::HuggingFace)
                        .map_err(|err| err.to_string())
                }
                HuggingFaceTokenizerSource::File => {
                    let tokenizer_file = spec.tokenizer_file.as_ref().ok_or_else(|| {
                        String::from("missing tokenizer file for hugging face backend")
                    })?;

                    Tokenizer::from_file(tokenizer_file)
                        .map(Self::HuggingFace)
                        .map_err(|err| err.to_string())
                }
            },
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

impl TokenizerSpec {
    pub fn hf_builtin(name: HfBuiltinTokenizer) -> Self {
        Self::HuggingFace {
            spec: HuggingFaceTokenizerSpec {
                source: HuggingFaceTokenizerSource::Builtin,
                name: Some(name),
                tokenizer_file: None,
            },
        }
    }

    pub fn hf_file(tokenizer_file: PathBuf) -> Self {
        Self::HuggingFace {
            spec: HuggingFaceTokenizerSpec {
                source: HuggingFaceTokenizerSource::File,
                name: None,
                tokenizer_file: Some(tokenizer_file),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::hf_registry::HfBuiltinTokenizer;

    use super::{
        BuiltinTokenizerId, OpenAiEncoding, TokenizerBackend, TokenizerConfig, TokenizerSpec,
    };

    const MIXED_SAMPLE_A: &str = "Hello，中文🙂tokenizer\nline2";
    const MIXED_SAMPLE_B: &str = "function_call({\"城市\":\"上海\",\"温度\":23.5})🚀";

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
        let mut backend = TokenizerBackend::from_spec(&TokenizerSpec::hf_file(hf_fixture()))
            .expect("hf tokenizer");

        let count = backend.count("hello world").expect("count");

        assert_eq!(count, 1);
    }

    #[test]
    fn builtin_huggingface_tokenizers_load() {
        let cases = [
            (HfBuiltinTokenizer::Qwen3, 8, 15),
            (HfBuiltinTokenizer::DeepseekV32, 10, 15),
            (HfBuiltinTokenizer::Glm5, 8, 16),
        ];

        for (name, expected_a, expected_b) in cases {
            let mut backend =
                TokenizerBackend::from_spec(&TokenizerSpec::hf_builtin(name)).expect("hf builtin");

            assert_eq!(backend.count(MIXED_SAMPLE_A).expect("count"), expected_a);
            assert_eq!(backend.count(MIXED_SAMPLE_B).expect("count"), expected_b);
        }
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
        let config = TokenizerConfig::parse_compare_spec("hf:tests/fixtures/hf-tokenizer.json")
            .expect("config");

        assert_eq!(config.label, "hf:hf-tokenizer.json");
        assert_eq!(
            config.spec,
            TokenizerSpec::hf_file(PathBuf::from("tests/fixtures/hf-tokenizer.json"))
        );
    }

    #[test]
    fn parse_hf_builtin_compare_spec_uses_builtin_name_as_label() {
        let config = TokenizerConfig::parse_compare_spec("hf_builtin:qwen3").expect("config");

        assert_eq!(config.label, "hf:qwen3");
        assert_eq!(
            config.spec,
            TokenizerSpec::hf_builtin(HfBuiltinTokenizer::Qwen3)
        );
    }

    #[test]
    fn parse_bare_builtin_compare_spec_uses_unified_id() {
        let config = TokenizerConfig::parse_compare_spec("qwen3").expect("config");

        assert_eq!(config.label, "hf:qwen3");
        assert_eq!(
            config.spec,
            BuiltinTokenizerId::Qwen3.into_tokenizer_config().spec
        );
    }

    #[test]
    fn parse_file_compare_spec_uses_file_prefix() {
        let config = TokenizerConfig::parse_compare_spec("file:tests/fixtures/hf-tokenizer.json")
            .expect("config");

        assert_eq!(config.label, "hf:hf-tokenizer.json");
        assert_eq!(
            config.spec,
            TokenizerSpec::hf_file(PathBuf::from("tests/fixtures/hf-tokenizer.json"))
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
