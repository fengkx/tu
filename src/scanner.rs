use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use content_inspector::{ContentType, inspect};
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use serde::Serialize;

use crate::tokenizer::TokenizerBackend;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum BinaryPolicy {
    Skip,
    Lossy,
    Error,
}

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub display_all: bool,
    pub max_depth: Option<usize>,
    pub binary_policy: BinaryPolicy,
    pub respect_ignore: bool,
    pub follow_links: bool,
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ScanRoot {
    Path(PathBuf),
    Stdin,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    File,
    Dir,
    Stdin,
}

#[derive(Debug, Clone, Serialize)]
pub struct EntryStat {
    pub path: String,
    pub kind: EntryKind,
    pub tokens: u64,
    pub files: u64,
    pub skipped: u64,
    pub errors: u64,
    pub depth: usize,
}

#[derive(Debug, Clone)]
pub struct RootScanResult {
    pub root: EntryStat,
    pub entries: Vec<EntryStat>,
    pub diagnostics: Vec<Diagnostic>,
}

impl RootScanResult {
    pub fn had_errors(&self) -> bool {
        self.root.errors > 0
    }
}

#[derive(Debug, Clone, Default)]
struct Aggregate {
    tokens: u64,
    files: u64,
    skipped: u64,
    errors: u64,
    depth: usize,
}

#[derive(Debug, Clone)]
struct FileEntry {
    path: PathBuf,
    tokens: u64,
    depth: usize,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DiagnosticLevel {
    Warning,
    Error,
}

impl DiagnosticLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

pub fn validate_excludes(patterns: &[String]) -> Result<(), String> {
    compile_excludes(patterns).map(|_| ())
}

pub fn scan_root(
    root: &ScanRoot,
    options: &ScanOptions,
    tokenizer: &mut TokenizerBackend,
    stdin_bytes: &[u8],
) -> RootScanResult {
    match root {
        ScanRoot::Stdin => scan_stdin(options, tokenizer, stdin_bytes),
        ScanRoot::Path(path) => scan_path(path, options, tokenizer),
    }
}

fn scan_stdin(
    options: &ScanOptions,
    tokenizer: &mut TokenizerBackend,
    stdin_bytes: &[u8],
) -> RootScanResult {
    let mut diagnostics = Vec::new();
    let root = match count_bytes(stdin_bytes, options.binary_policy, tokenizer) {
        CountOutcome::Counted(tokens) => EntryStat {
            path: String::from("-"),
            kind: EntryKind::Stdin,
            tokens,
            files: 1,
            skipped: 0,
            errors: 0,
            depth: 0,
        },
        CountOutcome::Skipped(reason) => {
            diagnostics.push(Diagnostic {
                level: DiagnosticLevel::Warning,
                message: format!("-: {reason}"),
            });
            EntryStat {
                path: String::from("-"),
                kind: EntryKind::Stdin,
                tokens: 0,
                files: 0,
                skipped: 1,
                errors: 0,
                depth: 0,
            }
        }
        CountOutcome::Failed(reason) => {
            diagnostics.push(Diagnostic {
                level: DiagnosticLevel::Error,
                message: format!("-: {reason}"),
            });
            EntryStat {
                path: String::from("-"),
                kind: EntryKind::Stdin,
                tokens: 0,
                files: 0,
                skipped: 0,
                errors: 1,
                depth: 0,
            }
        }
    };

    RootScanResult {
        entries: vec![root.clone()],
        root,
        diagnostics,
    }
}

fn scan_path(
    path: &Path,
    options: &ScanOptions,
    tokenizer: &mut TokenizerBackend,
) -> RootScanResult {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) => {
            return RootScanResult {
                root: EntryStat {
                    path: path.display().to_string(),
                    kind: EntryKind::File,
                    tokens: 0,
                    files: 0,
                    skipped: 0,
                    errors: 1,
                    depth: 0,
                },
                entries: vec![EntryStat {
                    path: path.display().to_string(),
                    kind: EntryKind::File,
                    tokens: 0,
                    files: 0,
                    skipped: 0,
                    errors: 1,
                    depth: 0,
                }],
                diagnostics: vec![Diagnostic {
                    level: DiagnosticLevel::Error,
                    message: format!("{}: {err}", path.display()),
                }],
            };
        }
    };

    if metadata.file_type().is_symlink() && !options.follow_links {
        let root = EntryStat {
            path: path.display().to_string(),
            kind: EntryKind::File,
            tokens: 0,
            files: 0,
            skipped: 1,
            errors: 0,
            depth: 0,
        };

        return RootScanResult {
            entries: vec![root.clone()],
            root,
            diagnostics: Vec::new(),
        };
    }

    if metadata.is_file() {
        return scan_single_file(path, options, tokenizer);
    }

    scan_directory(path, options, tokenizer)
}

fn scan_single_file(
    path: &Path,
    options: &ScanOptions,
    tokenizer: &mut TokenizerBackend,
) -> RootScanResult {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) => {
            let root = EntryStat {
                path: path.display().to_string(),
                kind: EntryKind::File,
                tokens: 0,
                files: 0,
                skipped: 0,
                errors: 1,
                depth: 0,
            };

            return RootScanResult {
                entries: vec![root.clone()],
                root,
                diagnostics: vec![Diagnostic {
                    level: DiagnosticLevel::Error,
                    message: format!("{}: {err}", path.display()),
                }],
            };
        }
    };

    let (root, diagnostics) = match count_bytes(&bytes, options.binary_policy, tokenizer) {
        CountOutcome::Counted(tokens) => (
            EntryStat {
                path: path.display().to_string(),
                kind: EntryKind::File,
                tokens,
                files: 1,
                skipped: 0,
                errors: 0,
                depth: 0,
            },
            Vec::new(),
        ),
        CountOutcome::Skipped(reason) => (
            EntryStat {
                path: path.display().to_string(),
                kind: EntryKind::File,
                tokens: 0,
                files: 0,
                skipped: 1,
                errors: 0,
                depth: 0,
            },
            vec![Diagnostic {
                level: DiagnosticLevel::Warning,
                message: format!("{}: {reason}", path.display()),
            }],
        ),
        CountOutcome::Failed(reason) => (
            EntryStat {
                path: path.display().to_string(),
                kind: EntryKind::File,
                tokens: 0,
                files: 0,
                skipped: 0,
                errors: 1,
                depth: 0,
            },
            vec![Diagnostic {
                level: DiagnosticLevel::Error,
                message: format!("{}: {reason}", path.display()),
            }],
        ),
    };

    RootScanResult {
        entries: vec![root.clone()],
        root,
        diagnostics,
    }
}

fn scan_directory(
    root: &Path,
    options: &ScanOptions,
    tokenizer: &mut TokenizerBackend,
) -> RootScanResult {
    let exclude_set =
        compile_excludes(&options.exclude).expect("exclude patterns validated earlier");
    let mut builder = WalkBuilder::new(root);
    builder.hidden(false);
    builder.follow_links(options.follow_links);
    builder.parents(options.respect_ignore);
    builder.git_ignore(options.respect_ignore);
    builder.git_global(options.respect_ignore);
    builder.git_exclude(options.respect_ignore);
    builder.ignore(options.respect_ignore);
    builder.require_git(false);

    let mut directories = BTreeMap::new();
    let mut files = Vec::new();
    let mut diagnostics = Vec::new();
    let mut unattached_errors = 0u64;

    directories.insert(
        root.to_path_buf(),
        Aggregate {
            depth: 0,
            ..Aggregate::default()
        },
    );

    for result in builder.build() {
        match result {
            Ok(entry) => {
                let path = entry.path();
                if should_exclude(root, path, &exclude_set) {
                    continue;
                }

                if entry.depth() == 0 {
                    continue;
                }

                let Some(file_type) = entry.file_type() else {
                    continue;
                };

                if file_type.is_dir() {
                    directories
                        .entry(path.to_path_buf())
                        .or_insert_with(|| Aggregate {
                            depth: entry.depth(),
                            ..Aggregate::default()
                        });
                    continue;
                }

                if !file_type.is_file() {
                    continue;
                }

                let bytes = match fs::read(path) {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        diagnostics.push(Diagnostic {
                            level: DiagnosticLevel::Error,
                            message: format!("{}: {err}", path.display()),
                        });
                        increment_ancestors(root, path, &mut directories, 0, 0, 0, 1);
                        continue;
                    }
                };

                match count_bytes(&bytes, options.binary_policy, tokenizer) {
                    CountOutcome::Counted(tokens) => {
                        files.push(FileEntry {
                            path: path.to_path_buf(),
                            tokens,
                            depth: entry.depth(),
                        });
                        increment_ancestors(root, path, &mut directories, tokens, 1, 0, 0);
                    }
                    CountOutcome::Skipped(reason) => {
                        diagnostics.push(Diagnostic {
                            level: DiagnosticLevel::Warning,
                            message: format!("{}: {reason}", path.display()),
                        });
                        increment_ancestors(root, path, &mut directories, 0, 0, 1, 0);
                    }
                    CountOutcome::Failed(reason) => {
                        diagnostics.push(Diagnostic {
                            level: DiagnosticLevel::Error,
                            message: format!("{}: {reason}", path.display()),
                        });
                        increment_ancestors(root, path, &mut directories, 0, 0, 0, 1);
                    }
                }
            }
            Err(err) => {
                diagnostics.push(Diagnostic {
                    level: DiagnosticLevel::Error,
                    message: err.to_string(),
                });
                unattached_errors += 1;
            }
        }
    }

    let mut entries = Vec::new();
    if options.display_all {
        entries.extend(files.into_iter().filter_map(|file| {
            if options
                .max_depth
                .is_some_and(|max_depth| file.depth > max_depth)
            {
                return None;
            }

            Some(EntryStat {
                path: file.path.display().to_string(),
                kind: EntryKind::File,
                tokens: file.tokens,
                files: 1,
                skipped: 0,
                errors: 0,
                depth: file.depth,
            })
        }));

        entries.extend(directories.iter().filter_map(|(path, aggregate)| {
            if options
                .max_depth
                .is_some_and(|max_depth| aggregate.depth > max_depth)
            {
                return None;
            }

            Some(EntryStat {
                path: path.display().to_string(),
                kind: EntryKind::Dir,
                tokens: aggregate.tokens,
                files: aggregate.files,
                skipped: aggregate.skipped,
                errors: aggregate.errors + u64::from(path == root) * unattached_errors,
                depth: aggregate.depth,
            })
        }));

        entries.sort_by(|left, right| {
            right
                .depth
                .cmp(&left.depth)
                .then_with(|| left.path.cmp(&right.path))
        });
    }

    let root_aggregate = directories.get(root).cloned().unwrap_or(Aggregate {
        depth: 0,
        ..Aggregate::default()
    });
    let root_entry = EntryStat {
        path: root.display().to_string(),
        kind: EntryKind::Dir,
        tokens: root_aggregate.tokens,
        files: root_aggregate.files,
        skipped: root_aggregate.skipped,
        errors: root_aggregate.errors + unattached_errors,
        depth: 0,
    };

    if !options.display_all {
        entries.push(root_entry.clone());
    }

    RootScanResult {
        root: root_entry,
        entries,
        diagnostics,
    }
}

fn increment_ancestors(
    root: &Path,
    path: &Path,
    directories: &mut BTreeMap<PathBuf, Aggregate>,
    tokens: u64,
    files: u64,
    skipped: u64,
    errors: u64,
) {
    let mut current = path.parent();
    while let Some(dir) = current {
        if !dir.starts_with(root) {
            break;
        }

        let depth = dir
            .strip_prefix(root)
            .map(component_count)
            .unwrap_or_default();

        let aggregate = directories
            .entry(dir.to_path_buf())
            .or_insert_with(|| Aggregate {
                depth,
                ..Aggregate::default()
            });
        aggregate.tokens += tokens;
        aggregate.files += files;
        aggregate.skipped += skipped;
        aggregate.errors += errors;

        if dir == root {
            break;
        }
        current = dir.parent();
    }
}

fn component_count(path: &Path) -> usize {
    if path.as_os_str().is_empty() {
        0
    } else {
        path.components().count()
    }
}

fn compile_excludes(patterns: &[String]) -> Result<GlobSet, String> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern).map_err(|err| err.to_string())?);
    }
    builder.build().map_err(|err| err.to_string())
}

fn should_exclude(root: &Path, path: &Path, exclude_set: &GlobSet) -> bool {
    if exclude_set.is_empty() {
        return false;
    }

    let relative = path.strip_prefix(root).unwrap_or(path);
    exclude_set.is_match(relative) || exclude_set.is_match(path)
}

enum CountOutcome {
    Counted(u64),
    Skipped(String),
    Failed(String),
}

fn count_bytes(
    bytes: &[u8],
    binary_policy: BinaryPolicy,
    tokenizer: &mut TokenizerBackend,
) -> CountOutcome {
    let content_type = inspect(bytes);
    match binary_policy {
        BinaryPolicy::Skip => match (content_type, std::str::from_utf8(bytes)) {
            (ContentType::BINARY, _) => CountOutcome::Skipped(String::from("skipped binary input")),
            (_, Ok(text)) => tokenizer
                .count(text)
                .map(CountOutcome::Counted)
                .unwrap_or_else(CountOutcome::Failed),
            (_, Err(_)) => CountOutcome::Skipped(String::from("skipped non-utf8 input")),
        },
        BinaryPolicy::Lossy => {
            let text = String::from_utf8_lossy(bytes);
            tokenizer
                .count(&text)
                .map(CountOutcome::Counted)
                .unwrap_or_else(CountOutcome::Failed)
        }
        BinaryPolicy::Error => match (content_type, std::str::from_utf8(bytes)) {
            (ContentType::BINARY, _) => {
                CountOutcome::Failed(String::from("binary input encountered"))
            }
            (_, Ok(text)) => tokenizer
                .count(text)
                .map(CountOutcome::Counted)
                .unwrap_or_else(CountOutcome::Failed),
            (_, Err(_)) => CountOutcome::Failed(String::from("non-utf8 input encountered")),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::{
        BinaryPolicy, DiagnosticLevel, RootScanResult, ScanOptions, ScanRoot, scan_root,
        validate_excludes,
    };
    use crate::tokenizer::{OpenAiEncoding, TokenizerBackend, TokenizerSpec};

    fn scan_options(binary_policy: BinaryPolicy) -> ScanOptions {
        ScanOptions {
            display_all: false,
            max_depth: None,
            binary_policy,
            respect_ignore: true,
            follow_links: false,
            exclude: Vec::new(),
        }
    }

    fn openai_backend() -> TokenizerBackend {
        TokenizerBackend::from_spec(&TokenizerSpec::OpenAi {
            encoding: OpenAiEncoding::O200kBase,
        })
        .expect("openai backend")
    }

    #[test]
    fn validate_excludes_rejects_invalid_glob() {
        assert!(validate_excludes(&[String::from("[")]).is_err());
    }

    #[test]
    fn scan_root_counts_explicit_file() {
        let tempdir = TempDir::new().expect("tempdir");
        let path = tempdir.path().join("file.txt");
        fs::write(&path, "hello world").expect("write file");

        let mut tokenizer = openai_backend();
        let result = scan_root(
            &ScanRoot::Path(path.clone()),
            &scan_options(BinaryPolicy::Skip),
            &mut tokenizer,
            &[],
        );

        assert_eq!(result.root.path, path.display().to_string());
        assert_eq!(result.root.tokens, 2);
        assert_eq!(result.root.files, 1);
        assert_eq!(result.root.skipped, 0);
        assert_eq!(result.root.errors, 0);
    }

    #[test]
    fn scan_root_returns_zero_for_empty_directory() {
        let tempdir = TempDir::new().expect("tempdir");
        let mut tokenizer = openai_backend();

        let result = scan_root(
            &ScanRoot::Path(tempdir.path().to_path_buf()),
            &scan_options(BinaryPolicy::Skip),
            &mut tokenizer,
            &[],
        );

        assert_eq!(result.root.path, tempdir.path().display().to_string());
        assert_eq!(result.root.tokens, 0);
        assert_eq!(result.root.files, 0);
        assert_eq!(result.root.errors, 0);
    }

    #[test]
    fn scan_root_respects_gitignore_for_top_level_file() {
        let tempdir = TempDir::new().expect("tempdir");
        fs::write(tempdir.path().join(".gitignore"), "ignored.txt\n").expect("write gitignore");
        fs::write(tempdir.path().join("ignored.txt"), "hello world").expect("write ignored file");

        let mut tokenizer = openai_backend();
        let mut options = scan_options(BinaryPolicy::Skip);
        options.exclude.push(String::from(".gitignore"));
        let result = scan_root(
            &ScanRoot::Path(tempdir.path().to_path_buf()),
            &options,
            &mut tokenizer,
            &[],
        );

        assert_eq!(result.root.tokens, 0);
        assert_eq!(result.root.files, 0);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn binary_skip_reports_warning_without_error_exit() {
        let result = scan_binary_file(BinaryPolicy::Skip);

        assert_eq!(result.root.tokens, 0);
        assert_eq!(result.root.skipped, 1);
        assert_eq!(result.root.errors, 0);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].level, DiagnosticLevel::Warning);
    }

    #[test]
    fn binary_error_reports_error() {
        let result = scan_binary_file(BinaryPolicy::Error);

        assert_eq!(result.root.tokens, 0);
        assert_eq!(result.root.skipped, 0);
        assert_eq!(result.root.errors, 1);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].level, DiagnosticLevel::Error);
    }

    fn scan_binary_file(binary_policy: BinaryPolicy) -> RootScanResult {
        let tempdir = TempDir::new().expect("tempdir");
        let path = tempdir.path().join("binary.bin");
        fs::write(&path, [0, 1, 2, 3]).expect("write binary file");

        let mut tokenizer = openai_backend();
        scan_root(
            &ScanRoot::Path(path),
            &scan_options(binary_policy),
            &mut tokenizer,
            &[],
        )
    }

    #[test]
    fn binary_lossy_counts_bytes() {
        let tempdir = TempDir::new().expect("tempdir");
        let path = tempdir.path().join("binary.bin");
        fs::write(&path, [0, 1, 2, 3]).expect("write binary file");

        let mut tokenizer = openai_backend();
        let result = scan_root(
            &ScanRoot::Path(path),
            &scan_options(BinaryPolicy::Lossy),
            &mut tokenizer,
            &[],
        );

        assert!(result.root.tokens > 0);
        assert_eq!(result.root.files, 1);
        assert!(result.diagnostics.is_empty());
    }
}
