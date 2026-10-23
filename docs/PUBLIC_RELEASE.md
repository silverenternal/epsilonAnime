# Public release checklist

Audit date: 2026-10-24

## Privacy scan

The tracked source was scanned for:

- NetEase cookies and `MUSIC_U` values;
- GitHub tokens, API keys and private keys;
- real account UIDs, nicknames, listening history and playlists;
- absolute home-directory and temporary-file paths;
- unredacted log and cache files;
- accidental `.env`, session, cookie and binary artifacts.

No real credentials or account data are tracked. Runtime logs, state, caches,
virtual environments and build output are ignored by `.gitignore`.

`detect-secrets` reported two high-entropy strings inside vendored pyncm:
the BASE62 alphabet and NetEase's public WEAPI RSA modulus. Both are upstream
protocol constants, not user secrets, and were manually reviewed.

## Repository hygiene

- MIT license added for original project code.
- Vendored `pyncm` keeps its Apache-2.0 license and is listed in
  `docs/THIRD_PARTY.md`.
- Local commit identity uses the GitHub noreply address rather than a personal
  email.
- Security reporting and credential-handling expectations are documented in
  `SECURITY.md`.
- CI uses read-only repository permissions.
- Dependabot monitors Cargo, Python and GitHub Actions dependencies.

## Pre-publish verification

```bash
cargo fmt --all -- --check
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo audit
./scripts/check_file_lengths.sh
cd py
uv run python -m unittest discover -s tests -v
uv run --with pip-audit pip-audit
```

Before pushing, inspect `git diff --cached` and run a secret scanner against
the staged tree.
