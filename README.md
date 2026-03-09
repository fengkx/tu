# tu

`tu` is a small command-line tool for counting tokens in files and directories.

It is intentionally shaped like `du`, but the unit is not disk usage. The unit is the number of tokens produced by a tokenizer.

By default, `tu`:

- uses OpenAI `o200k_base`
- respects `.gitignore`, `.ignore`, and git exclude rules
- skips binary files with a warning
- prints one summary line per input root

## Why

When working with LLM prompts, repositories, or corpora, byte size is often less useful than token count. `tu` answers questions like:

- "How many tokens are in this prompt file?"
- "How large is this repository in `o200k_base` terms?"
- "Which subtree is contributing the most tokens?"
- "What would this look like under a HuggingFace tokenizer?"

## Installation

Build from source:

```bash
cargo build --release
```

The binary will be available at:

```bash
./target/release/tu
```

Or install it into Cargo's binary directory:

```bash
cargo install --path .
```

## Quick Start

Count tokens in the current directory:

```bash
tu
```

Count tokens in a single file:

```bash
tu prompt.txt
```

Read from stdin:

```bash
cat prompt.txt | tu
```

Show every file and directory aggregate:

```bash
tu --all .
```

Use human-readable units:

```bash
tu --human .
```

Emit JSON for scripts:

```bash
tu --json .
```

## Behavior

### Traversal

- If no path is provided and stdin is a TTY, `tu` scans `.`.
- If no path is provided and stdin is piped, `tu` reads stdin.
- You can use `-` explicitly to read stdin alongside file paths.
- Symbolic links are not followed unless `--follow-links` is enabled.

### Ignore rules

By default, `tu` respects:

- `.gitignore`
- `.ignore`
- `.git/info/exclude`
- global git ignore rules

Disable this behavior with:

```bash
tu --no-ignore .
```

Add extra exclusions with repeatable glob patterns:

```bash
tu --exclude '*.min.js' --exclude 'dist/**' .
```

### Binary files

Binary handling is controlled with `--binary`:

- `skip` (default): skip binary or non-UTF-8 input and print a warning
- `lossy`: decode with UTF-8 lossy conversion and still count tokens
- `error`: fail the command when binary or non-UTF-8 input is encountered

Examples:

```bash
tu --binary skip .
tu --binary lossy archive.dat
tu --binary error .
```

## Tokenizers

### Default OpenAI backend

The default backend is `openai` with `o200k_base`:

```bash
tu .
```

You can switch encodings:

```bash
tu --encoding cl100k_base .
tu --encoding p50k_base .
tu --encoding r50k_base .
```

Available OpenAI encodings:

- `o200k_base`
- `cl100k_base`
- `p50k_base`
- `r50k_base`

### HuggingFace backend

You can also point `tu` at a local `tokenizer.json`:

```bash
tu --tokenizer hf --tokenizer-file ./tokenizer.json .
```

This is useful when you want counts for a model-specific tokenizer outside the OpenAI family.

## Output

### Default text output

The default text format is:

```text
<tokens>\t<path>
```

Examples:

```bash
tu README.md
tu --all src
tu --total packages/a packages/b
```

### JSON output

Use `--json` when you need machine-readable output:

```bash
tu --json .
```

The JSON payload includes:

- `tokenizer`: the tokenizer configuration used for the run
- `entries`: emitted entries
- `total`: summed totals across roots
- `had_errors`: whether any execution error occurred

## Options

```text
Usage: tu [OPTIONS] [PATH]...

Arguments:
  [PATH]...  Files or directories to scan. Use `-` to read stdin

Options:
  -a, --all                    Output every file and directory aggregate
  -s, --summarize              Output only the summary for each root input
  -d, --max-depth <N>          Limit displayed depth. Deeper descendants are still counted in aggregates
      --tokenizer <TOKENIZER>  Select the tokenizer backend [default: openai] [possible values: openai, hf]
      --encoding <ENCODING>    Select the OpenAI encoding [default: o200k_base] [possible values: o200k_base, cl100k_base, p50k_base, r50k_base]
      --tokenizer-file <PATH>  Path to a HuggingFace tokenizer.json
      --binary <BINARY>        Binary file handling policy [default: skip] [possible values: skip, lossy, error]
      --no-ignore              Disable .gitignore, .ignore, and git exclude rules
      --exclude <GLOB>         Exclude matching paths. Repeatable
  -L, --follow-links           Follow symbolic links
  -H, --human                  Print human-readable token units
      --json                   Emit JSON instead of text output
      --total                  Print a total row when multiple roots are provided
  -h, --help                   Print help
  -V, --version                Print version
```

## Examples

Show only a top-level summary for the current repository:

```bash
tu .
```

Inspect a subtree in detail:

```bash
tu --all --max-depth 2 src
```

Compare two directories and print a combined total:

```bash
tu --total app server
```

Count a prompt under a different tokenizer:

```bash
tu --encoding cl100k_base prompt.md
```

Count with a HuggingFace tokenizer:

```bash
tu --tokenizer hf --tokenizer-file ./tokenizer.json docs
```

Use in shell pipelines:

```bash
git show HEAD~1:README.md | tu
```

Process JSON output with `jq`:

```bash
tu --json . | jq '.total.tokens'
```

## Exit Codes

- `0`: success
- `1`: the command completed but at least one scan/read/count error occurred
- `2`: invalid configuration or startup failure

Warnings, such as skipped binary files, are written to stderr.

## Notes

- Token counts depend on the selected tokenizer. Different backends or encodings will produce different numbers.
- `--max-depth` only affects displayed entries. Deeper files still contribute to ancestor aggregates.
- `--summarize` exists for clarity, but summary-only output is already the default.

## Contributing

Use `just` to set up the local environment and run checks:

```bash
just ready
```

Useful recipes:

```bash
just venv   # create .venv if it does not exist
just lock   # refresh uv.lock
just sync   # install Python dev dependencies into .venv
just test   # run the full Rust test suite
```

`just ready` is the recommended entrypoint for contributors. It ensures the project virtualenv exists, synchronizes the Python test dependency used by integration tests, and then runs `cargo test`.

## Release

Preview a release without changing the repository:

```bash
just release-plan <version>
```

Create the local release commit, tag, and changelog:

```bash
just release <version>
```

Push the branch and tag to GitHub:

```bash
just publish-release <version>
```

These commands run `just ready` first, update the version, regenerate `CHANGELOG.md`, create a `release: vX.Y.Z` commit, and create an annotated `vX.Y.Z` tag. After the tag is pushed, GitHub Actions creates the GitHub Release and uploads the platform archives.
