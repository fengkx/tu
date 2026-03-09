set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

project_root := justfile_directory()
venv_dir := project_root + "/.venv"
venv_python := venv_dir + "/bin/python"

venv:
    @if [ ! -x "{{venv_python}}" ]; then \
        echo "Creating virtualenv at {{venv_dir}}"; \
        uv venv "{{venv_dir}}"; \
    fi

lock: venv
    VIRTUAL_ENV="{{venv_dir}}" UV_PYTHON="{{venv_python}}" uv lock

sync: venv
    VIRTUAL_ENV="{{venv_dir}}" UV_PYTHON="{{venv_python}}" uv sync --active --locked --group dev

test: sync
    cargo test --quiet

ready: test

release-tools:
    @if ! command -v cargo-release >/dev/null 2>&1; then \
        echo "Installing cargo-release"; \
        cargo install --locked cargo-release; \
    fi
    @if ! command -v git-cliff >/dev/null 2>&1; then \
        echo "Installing git-cliff"; \
        cargo install --locked git-cliff; \
    fi

_release-checks version:
    @if [[ ! "{{version}}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then \
        echo "error: version must be in X.Y.Z format" >&2; \
        exit 1; \
    fi
    @if [ "$(git branch --show-current)" != "master" ]; then \
        echo "error: releases must run from master" >&2; \
        exit 1; \
    fi
    @if [ -n "$(git status --short)" ]; then \
        echo "error: working tree must be clean before releasing" >&2; \
        exit 1; \
    fi
    @if git rev-parse -q --verify "refs/tags/v{{version}}" >/dev/null 2>&1; then \
        echo "error: tag v{{version}} already exists locally" >&2; \
        exit 1; \
    fi

_publish-checks version:
    @if [[ ! "{{version}}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then \
        echo "error: version must be in X.Y.Z format" >&2; \
        exit 1; \
    fi
    @if [ "$(git branch --show-current)" != "master" ]; then \
        echo "error: releases must run from master" >&2; \
        exit 1; \
    fi
    @if [ -n "$(git status --short)" ]; then \
        echo "error: working tree must be clean before publishing" >&2; \
        exit 1; \
    fi
    @if ! git rev-parse -q --verify "refs/tags/v{{version}}" >/dev/null 2>&1; then \
        echo "error: local tag v{{version}} does not exist" >&2; \
        exit 1; \
    fi
    @if [ "$(git rev-list -n 1 "v{{version}}")" != "$(git rev-parse HEAD)" ]; then \
        echo "error: tag v{{version}} must point at HEAD before publishing" >&2; \
        exit 1; \
    fi

release-changelog-hook:
    @if [ -z "${NEW_VERSION:-}" ]; then \
        echo "error: NEW_VERSION is required" >&2; \
        exit 1; \
    elif [ "${DRY_RUN:-false}" = "true" ]; then \
        echo "Skipping changelog generation during dry run"; \
    else \
        VIRTUAL_ENV="{{venv_dir}}" UV_PYTHON="{{venv_python}}" uv lock; \
        git-cliff --config cliff.toml --tag "v${NEW_VERSION}" --output CHANGELOG.md; \
    fi

release-plan version: release-tools
    just _release-checks {{version}}
    just ready
    @echo
    @echo "==> Changelog preview for v{{version}}"
    git-cliff --config cliff.toml --unreleased --tag "v{{version}}"
    @echo
    @echo "==> cargo-release dry run for v{{version}}"
    cargo release {{version}} --no-confirm --no-verify --no-publish --no-push

release version: release-tools
    just _release-checks {{version}}
    just ready
    cargo release {{version}} --execute --no-confirm --no-verify --no-publish --no-push

publish-release version: release-tools
    @if git rev-parse -q --verify "refs/tags/v{{version}}" >/dev/null 2>&1; then \
        just _publish-checks {{version}}; \
    else \
        just release {{version}}; \
    fi
    git push origin master
    git push origin "v{{version}}"
