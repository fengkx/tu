use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use assert_cmd::Command;
use tempfile::TempDir;

pub fn tempdir() -> TempDir {
    TempDir::new().expect("tempdir")
}

pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

pub fn cargo_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tu"))
}

pub fn python_tiktoken_count(encoding: &str, text: &str) -> u64 {
    let python = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".venv")
        .join("bin")
        .join("python");

    assert!(
        python.is_file(),
        "expected python interpreter at {}",
        python.display()
    );

    let output = ProcessCommand::new(&python)
        .arg("-c")
        .arg(
            r#"import sys
try:
    import tiktoken
except ImportError as err:
    raise SystemExit(f"failed to import tiktoken: {err}")

encoding = tiktoken.get_encoding(sys.argv[1])
print(len(encoding.encode(sys.argv[2])))
"#,
        )
        .arg(encoding)
        .arg(text)
        .output()
        .expect("run python tiktoken");

    assert!(
        output.status.success(),
        "python tiktoken command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout)
        .expect("utf8 stdout")
        .trim()
        .parse()
        .expect("numeric token count")
}

pub fn write_text(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(path, contents).expect("write text");
}

pub fn write_bytes(path: impl AsRef<Path>, contents: &[u8]) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(path, contents).expect("write bytes");
}
