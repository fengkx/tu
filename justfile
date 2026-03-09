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
