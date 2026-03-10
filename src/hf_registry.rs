use clap::ValueEnum;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum HfBuiltinTokenizer {
    #[serde(rename = "qwen3")]
    #[value(name = "qwen3")]
    Qwen3,
    #[serde(rename = "deepseek_v3_2")]
    #[value(name = "deepseek_v3_2")]
    DeepseekV32,
    #[serde(rename = "glm5")]
    #[value(name = "glm5")]
    Glm5,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BuiltinHfSpec {
    pub name: HfBuiltinTokenizer,
    pub repo: &'static str,
    pub revision: &'static str,
    pub license: &'static str,
    pub sha256: &'static str,
    compressed_bytes: &'static [u8],
}

const QWEN3_TOKENIZER_JSON: &[u8] =
    include_flate::codegen::deflate_file!("assets/hf/qwen3/tokenizer.json");
const DEEPSEEK_V3_2_TOKENIZER_JSON: &[u8] =
    include_flate::codegen::deflate_file!("assets/hf/deepseek_v3_2/tokenizer.json");
const GLM5_TOKENIZER_JSON: &[u8] =
    include_flate::codegen::deflate_file!("assets/hf/glm5/tokenizer.json");

impl HfBuiltinTokenizer {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Qwen3 => "qwen3",
            Self::DeepseekV32 => "deepseek_v3_2",
            Self::Glm5 => "glm5",
        }
    }

    pub fn spec(self) -> BuiltinHfSpec {
        match self {
            Self::Qwen3 => BuiltinHfSpec {
                name: self,
                repo: "Qwen/Qwen3-32B",
                revision: "9216db5781bf21249d130ec9da846c4624c16137",
                license: "apache-2.0",
                sha256: "aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4",
                compressed_bytes: QWEN3_TOKENIZER_JSON,
            },
            Self::DeepseekV32 => BuiltinHfSpec {
                name: self,
                repo: "deepseek-ai/DeepSeek-V3.2",
                revision: "a7e62ac04ecb2c0a54d736dc46601c5606cf10a6",
                license: "mit",
                sha256: "cd050be35cae877f8f0aa847f45aa87e23835a56ca32b29b28545597852784e5",
                compressed_bytes: DEEPSEEK_V3_2_TOKENIZER_JSON,
            },
            Self::Glm5 => BuiltinHfSpec {
                name: self,
                repo: "zai-org/GLM-5",
                revision: "b8a9fc5d565cd6bb71851c3e76f302f4b74d8c64",
                license: "mit",
                sha256: "19e773648cb4e65de8660ea6365e10acca112d42a854923df93db4a6f333a82d",
                compressed_bytes: GLM5_TOKENIZER_JSON,
            },
        }
    }

    pub fn load_bytes(self) -> Result<Vec<u8>, String> {
        self.spec().load_bytes()
    }
}

impl BuiltinHfSpec {
    pub fn load_bytes(self) -> Result<Vec<u8>, String> {
        Ok(include_flate::decode(self.compressed_bytes, None))
    }
}
