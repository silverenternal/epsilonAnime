# Contributing

## Development setup

```bash
cargo build
cd py
uv sync --locked
cd ..
./scripts/run.sh --demo
```

## Required checks

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cd py
uv run python -m unittest discover -s tests -v
cd ..
./scripts/check_file_lengths.sh
```

Every Rust or Python source file must remain at or below 600 lines. Split
modules instead of raising this limit.

## Privacy requirements

Do not include real NetEase cookies, account UIDs, nicknames, listening
history, IP addresses, absolute home-directory paths, backend logs, or
unredacted streaming URLs in code, tests, issues, screenshots, or commits.
Use synthetic examples and reserved domains such as `example.test`.

## Pull requests

Keep changes focused, add regression tests for behavioral fixes, update
relevant documentation, and ensure the three-platform CI matrix remains green.
