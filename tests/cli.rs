mod common;

use std::path::Path;

use predicates::prelude::*;
use serde_json::Value;

use common::{cargo_bin, fixture_path, python_tiktoken_count, tempdir, write_bytes, write_text};

fn stdin_json_tokens(input: &str) -> u64 {
    let output = cargo_bin()
        .args(["--json", "-"])
        .write_stdin(input)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("utf8 stdout");
    let json: Value = serde_json::from_str(&stdout).expect("parse json");

    json["entries"][0]["tokens"]
        .as_u64()
        .expect("stdin token count")
}

fn compare_json(input: &str, specs: &[&str]) -> Value {
    let mut command = cargo_bin();
    command.arg("--json");
    for spec in specs {
        command.arg("--compare").arg(spec);
    }

    let output = command
        .arg("-")
        .write_stdin(input)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("utf8 stdout");

    serde_json::from_str(&stdout).expect("parse json")
}

#[test]
fn respects_gitignore_by_default() {
    let tempdir = tempdir();
    write_text(tempdir.path().join(".gitignore"), "ignored.txt\n");
    write_text(tempdir.path().join("visible.txt"), "hello world");
    write_text(tempdir.path().join("ignored.txt"), "hello world");

    cargo_bin()
        .arg("--all")
        .arg(tempdir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("visible.txt"))
        .stdout(predicate::str::contains("ignored.txt").not())
        .stdout(predicate::str::contains(
            tempdir.path().to_str().expect("utf8 path"),
        ));
}

#[test]
fn no_ignore_includes_gitignored_files() {
    let tempdir = tempdir();
    write_text(tempdir.path().join(".gitignore"), "ignored.txt\n");
    write_text(tempdir.path().join("visible.txt"), "hello world");
    write_text(tempdir.path().join("ignored.txt"), "hello world");

    cargo_bin()
        .args(["--no-ignore", "--all"])
        .arg(tempdir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("visible.txt"))
        .stdout(predicate::str::contains("ignored.txt"));
}

#[test]
fn all_with_max_depth_hides_deeper_entries_but_keeps_aggregate() {
    let tempdir = tempdir();
    let nested = tempdir.path().join("level1").join("level2");
    write_text(nested.join("deep.txt"), "hello world");

    let output = cargo_bin()
        .args(["--all", "--max-depth", "1"])
        .arg(tempdir.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("utf8 stdout");

    assert!(stdout.contains(&tempdir.path().display().to_string()));
    assert!(stdout.contains("level1"));
    assert!(!stdout.contains("level2"));
    assert!(!stdout.contains("deep.txt"));
    assert!(
        stdout
            .lines()
            .any(|line| line.ends_with(tempdir.path().to_str().expect("utf8 path")))
    );
}

#[test]
fn total_adds_summary_for_multiple_roots() {
    let tempdir = tempdir();
    let first = tempdir.path().join("first.txt");
    let second = tempdir.path().join("second.txt");
    write_text(&first, "hello");
    write_text(&second, "world");

    let output = cargo_bin()
        .arg("--total")
        .arg(&first)
        .arg(&second)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("utf8 stdout");

    assert!(stdout.contains("first.txt"));
    assert!(stdout.contains("second.txt"));
    assert!(stdout.contains("\ttotal"));
    assert_eq!(stdout.lines().count(), 3);
}

#[test]
fn reads_stdin_when_no_paths_are_provided() {
    cargo_bin()
        .write_stdin("hello world")
        .assert()
        .success()
        .stdout(predicate::str::contains("\t-\n"));
}

#[test]
fn reads_stdin_for_explicit_dash_path() {
    let tempdir = tempdir();
    let file = tempdir.path().join("file.txt");
    write_text(&file, "hello");

    let output = cargo_bin()
        .arg("-")
        .arg(&file)
        .write_stdin("hello world")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("utf8 stdout");

    assert!(stdout.contains("\t-"));
    assert!(stdout.contains("file.txt"));
}

#[test]
fn json_output_is_parseable_and_contains_expected_fields() {
    let tempdir = tempdir();
    write_text(tempdir.path().join("file.txt"), "hello world");

    let output = cargo_bin()
        .arg("--json")
        .arg(tempdir.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("utf8 stdout");
    let json: Value = serde_json::from_str(&stdout).expect("parse json");

    assert_eq!(json["tokenizer"]["kind"], "open_ai");
    assert!(json["entries"].is_array());
    assert!(json["total"].is_object());
    assert_eq!(json["had_errors"], false);
}

#[test]
fn binary_error_exits_non_zero_and_prints_stderr() {
    let tempdir = tempdir();
    let file = tempdir.path().join("binary.bin");
    write_bytes(&file, &[0, 1, 2, 3]);

    cargo_bin()
        .args(["--binary", "error"])
        .arg(&file)
        .assert()
        .failure()
        .stderr(predicate::str::contains("binary input encountered"));
}

#[test]
fn binary_skip_warns_but_does_not_fail() {
    let tempdir = tempdir();
    let file = tempdir.path().join("binary.bin");
    write_bytes(&file, &[0, 1, 2, 3]);

    cargo_bin()
        .arg(&file)
        .assert()
        .success()
        .stderr(predicate::str::contains("warning:"))
        .stderr(predicate::str::contains("skipped binary input"));
}

#[test]
fn binary_lossy_produces_non_zero_output() {
    let tempdir = tempdir();
    let file = tempdir.path().join("binary.bin");
    write_bytes(&file, &[0, 1, 2, 3]);

    let output = cargo_bin()
        .args(["--binary", "lossy"])
        .arg(&file)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("utf8 stdout");

    let tokens = stdout
        .lines()
        .next()
        .and_then(|line| line.split('\t').next())
        .expect("token column")
        .parse::<u64>()
        .expect("numeric tokens");
    assert!(tokens > 0);
}

#[test]
fn missing_path_returns_execution_error() {
    cargo_bin()
        .arg(Path::new("missing-file.txt"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("No such file").or(predicate::str::contains("os error")));
}

#[test]
fn invalid_exclude_returns_configuration_error() {
    cargo_bin()
        .args(["--exclude", "["])
        .arg(".")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error:"));
}

#[test]
fn hf_tokenizer_requires_tokenizer_file() {
    cargo_bin()
        .args(["--tokenizer", "hf", "."])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--tokenizer-file is required"));
}

#[test]
fn human_output_uses_compact_units() {
    let tempdir = tempdir();
    let repeated = "hello world ".repeat(800);
    write_text(tempdir.path().join("large.txt"), &repeated);

    cargo_bin()
        .args(["--human"])
        .arg(tempdir.path())
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"[0-9]+(\.[0-9])?[KMBT]\t").expect("regex"));
}

#[test]
fn hf_backend_counts_using_fixture_tokenizer() {
    let tempdir = tempdir();
    write_text(tempdir.path().join("sample.txt"), "hello world");

    let output = cargo_bin()
        .args([
            "--tokenizer",
            "hf",
            "--tokenizer-file",
            fixture_path("hf-tokenizer.json")
                .to_str()
                .expect("utf8 fixture"),
        ])
        .arg(tempdir.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("utf8 stdout");

    assert!(stdout.starts_with("1\t"));
}

#[test]
fn compare_text_output_uses_wide_table() {
    let output = cargo_bin()
        .args([
            "--compare",
            "openai:o200k_base",
            "--compare",
            "openai:cl100k_base",
            "-",
        ])
        .write_stdin("中文")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("utf8 stdout");

    assert!(stdout.starts_with("path\to200k_base\tcl100k_base\n"));
    assert!(stdout.contains("-\t1\t2\n"));
}

#[test]
fn compare_json_output_supports_mixed_tokenizers() {
    let hf_spec = format!(
        "hf:{}",
        fixture_path("hf-tokenizer.json")
            .to_str()
            .expect("utf8 fixture")
    );
    let json = compare_json("hello world", &["openai:o200k_base", &hf_spec]);

    assert_eq!(json["tokenizers"][0]["label"], "o200k_base");
    assert_eq!(json["tokenizers"][1]["label"], "hf:hf-tokenizer.json");
    assert_eq!(json["results"][0]["total"]["tokens"], 2);
    assert_eq!(json["results"][1]["total"]["tokens"], 1);
    assert_eq!(json["had_errors"], false);
}

#[test]
fn compare_rejects_single_tokenizer_flags() {
    cargo_bin()
        .args([
            "--compare",
            "openai:o200k_base",
            "--encoding",
            "cl100k_base",
            ".",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "--compare cannot be used with --tokenizer, --encoding, or --tokenizer-file",
        ));
}

#[test]
fn compare_stdin_matches_python_for_multiple_openai_encodings() {
    let input = "中文\n";
    let json = compare_json(input, &["openai:o200k_base", "openai:cl100k_base"]);
    let expected_o200k = python_tiktoken_count("o200k_base", input);
    let expected_cl100k = python_tiktoken_count("cl100k_base", input);

    assert_eq!(json["results"][0]["label"], "o200k_base");
    assert_eq!(json["results"][0]["total"]["tokens"], expected_o200k);
    assert_eq!(json["results"][1]["label"], "cl100k_base");
    assert_eq!(json["results"][1]["total"]["tokens"], expected_cl100k);
}

#[test]
fn stdin_chinese_without_newline_matches_python_tiktoken() {
    let input = "中文";
    let expected = python_tiktoken_count("o200k_base", input);

    assert_eq!(expected, 1);
    assert_eq!(stdin_json_tokens(input), expected);
}

#[test]
fn stdin_chinese_with_trailing_newline_matches_python_tiktoken() {
    let input = "中文\n";
    let expected = python_tiktoken_count("o200k_base", input);

    assert_eq!(expected, 2);
    assert_eq!(stdin_json_tokens(input), expected);
}
