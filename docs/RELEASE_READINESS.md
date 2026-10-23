# Release readiness: T301–T315

This record maps every active item in `todo.json` to its implementation and
repeatable verification. The engineering limit is enforced by
`scripts/check_file_lengths.sh`: no Rust, Python, or build source file may
exceed 600 lines.

## State, playback and discovery

| Task | Implementation | Verification |
| --- | --- | --- |
| T301 | `src/state_store.rs` stores versioned JSON envelopes in the user config directory, performs fsync + atomic replacement (including `MoveFileExW` on Windows), migrates compatible older schemas, and backs up corrupt/incompatible files. | State-store tests cover migration, corruption backup, and round-trip writes without leftover temp files. State paths are outside `target/`, so `cargo clean` is non-destructive. |
| T302 | `py/backend/player.py` supports HTTP Range seeking, ReplayGain peak-safe normalization, device selection, and null-device analysis fallback. Near track end, Rust starts a prepared backend which preloads metadata, cover, lyrics and the first decoder stream; `start` schedules it without repeating network setup. | Python tests cover Range/EOF behavior, gain normalization, device defaults and stream preloading. Real `--analyze-only` decoded the sample track after the seek endpoint regression was fixed. |
| T303 | `src/lyrics.rs` keeps the full timeline, scroll/search state, persisted translation/romanization/offset preferences, and continuously advances character highlighting inside timed segments. | Tests cover CJK width, scrolling, translated and romanized search, long-line clipping and segment progress. |
| T304 | `py/backend/library_api.py` normalizes song, album, artist, playlist, MV and radio collections, detail pages and related playlists. Rust keeps nested navigation, collection metadata, search facets and favorites. | Unit tests cover song/faceted search, artist details, pagination and offline collection paths; a live song search returned 221 normalized results. |
| T305 | `py/backend/offline.py` stores search/browse/collection responses, discovery history and recent tracks with bounded retention. Cached events are marked in Rust and survive injected network failures. | Python tests inject a network exception and verify cached fallback, history, recent tracks and cache clearing. |
| T306 | Media URLs are stripped from persistent prefetch records; startup scrubs older URL-bearing records. Invalid login state clears its cookie file, logout clears media/offline caches, and nested JSON/URL query logs are redacted. | Cache/token tests, expired-login tests, and nested JSON redaction tests pass. |

## Rendering and interaction

| Task | Implementation | Verification |
| --- | --- | --- |
| T307 | Animation preferences use stable string IDs. Profiles combine BPM range, energy and motion family; low-quality mode excludes high-cost kernels. Crossfade blends `FieldSample` intensity, depth and macro-generated light. | Policy tests verify stable favorites, disabled-mode stepping and low-end filtering. Adaptive tests verify bounded fields, depth and 3D light variation. |
| T308 | `src/background.rs` uses thread-local reusable scratch buffers. Terminal writes use a bounded channel and report backpressure. Stable metrics include sample/encode/write costs, dirty tiles and adaptive quality. | Metrics schema test plus release benchmark; interactive demo reported ~26.7 fps at 30 fps target with sample/encode/write timing. |
| T309 | `KittyCache` keeps one encoded 512×512 payload per cover, uses place-existing for moves, and supports opt-in ACK retransmit/timeout fallback to braille. | Cache test proves pixels are transferred once while a moved panel reuses the image ID. |
| T310 | `ShellWorkspace` supports multiple real PTYs/tabs, scrollback search, persisted bounded history, host paste, mouse protocol forwarding, selection copy and OSC8 URI capture/open. | Tests open/switch/close tabs, search old scrollback and parse OSC8 links; the Unix integration test executes a command through a real PTY. |
| T311 | Floating windows snap to screen/peer edges, resolve partial collisions, show a drag preview, preserve deliberate full stacking for glass composition, and restore arbitrary minimized windows. Layout snapshots are versioned. | Layout tests cover clamping, snapping, previews, glass overlap, presets and arbitrary minimize/restore. |
| T312 | Backend/auth errors expose code/scope/retryable metadata. Notifications persist through the state store. A panic hook appends location/payload to `target/crash.log`; `--stats` emits stable JSON at shutdown. | Notification classification/persistence path and metrics schema are tested. |
| T313 | Locale titles support Chinese/English/Japanese; high-contrast, deuteranopia and tritanopia palettes bypass cover-derived palette drift. Hotkeys and accessibility are TOML settings, with safe in-process reload. | Config parsing and locale cycling tests; all text remains contrast-repaired. |
| T314 | Proptest covers malformed NDJSON, arbitrary PCM and arbitrary key events; Python property tests feed random multilingual lyric data. | `cargo test --all-targets --all-features` and the Python suite run these tests on every CI run. |
| T315 | CI is a three-OS matrix for Rust and Python, adds LCOV coverage, dependency audits, ANSI/PTY golden coverage and per-OS artifacts. | Local release build, fmt, clippy, Rust/Python suites, file-length check and both dependency audits pass. Cross-OS jobs are defined in `.github/workflows/ci.yml`. |

## Final functional verification

- Real NetEase track `3347119767`: title/artist metadata, 512×512 RGB cover,
  83,984-cover-particle index, comments, Japanese lyrics with translation and
  romanization, and live feature events all arrived.
- The real decoder regression (`miniaudio -203` caused by a zero-length HTTP
  Range request when MP3 inspected EOF/ID3) was reproduced, fixed by tracking
  logical source position and treating total-length seeks as EOF, then verified
  with a live five-second decode.
- Interactive `--demo --stats` entered the alternate screen, rendered the 3D
  scene/layout, handled quit, restored terminal modes, and emitted valid JSON
  metrics.

## Repeatable commands

```bash
cargo fmt --all -- --check
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
cargo audit
./scripts/check_file_lengths.sh

cd py
uv run python -m compileall -q backend
uv run python -m unittest discover -s tests -v
uv run --with pip-audit pip-audit
```
