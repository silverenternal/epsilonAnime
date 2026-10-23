# Third-party references

## Chafa

- Project: https://github.com/hpjansson/chafa
- Reference revision: `413b76e4ddefc3875b5220c68a328651f235d3ae`
- License: LGPL-3.0-or-later for the Chafa library.

Chafa was cloned and studied as the reference for terminal glyph taxonomy: ASCII,
block, quadrant, braille and mixed symbol profiles, coverage-based glyph weighting,
and the tradeoff between glyph density and terminal font compatibility.

The `src/glyphs.rs` profile tables in this repository are a small, hand-curated
subset derived from Unicode block/braille semantics. No Chafa source files are
vendored or copied into the project.

## pyncm

- Snapshot: `Code-MonkeyZhang/pyncm` 1.8.1
- Vendored path: `py/vendor/pyncm`
- License: Apache-2.0; see `py/vendor/PYNCM_LICENSE`.

The QR flow uses `LoginQrcodeUnikey`, `GetLoginQRCodeUrl` and
`LoginQrcodeCheck`; profile and library views use `GetCurrentLoginStatus`,
`GetUserPlaylists` and `GetPlaylistAllTracks`.

## Python qrcode

- Project: https://github.com/lincolnloop/python-qrcode
- Version: `qrcode>=8,<9`
- License: BSD-3-Clause

The library generates the QR module matrix. The backend packs a 2×4 module
group into each Braille cell, preserving square modules while fitting a
scannable QR code in short terminals without reimplementing QR encoding.

## Embedded shell

- `portable-pty` 0.9.0 — cross-platform PTY interface from the WezTerm project.
  License: MIT.
- `vt100` 0.18.1 — ANSI terminal state parser used for the embedded shell
  screen and colors. License: MIT.

The application keeps the shell process alive when its panel is hidden. It
uses the user's `$SHELL`, converts crossterm key events to terminal byte
sequences, and renders the parsed vt100 screen through the existing cell buffer.

## Terminal graphics and clipboard

- `base64` 0.22 — MIT OR Apache-2.0, used to encode raw RGB cover pixels for
  the Kitty graphics protocol.
- `crossterm` 0.29 `osc52` feature — MIT, used for clipboard copy through the
  terminal host.

Kitty graphics remains opt-in/auto-detected and falls back to the portable
braille renderer. The application only deletes its own image id.

## Mathematical libraries

- `puruspe` 0.4.4 — Bessel functions, gamma, erf and related special functions.
  License: MIT OR Apache-2.0.
- `sphrs` 0.2.2 — real spherical and solid harmonics. License: MIT OR Apache-2.0.
- `glam` 0.34.1 — 3D vectors, matrices and rotations. License: MIT OR Apache-2.0.
- `rustfft` 6.4.x — FFT implementation used by the stream analyzer.
- `wide` 1.7.x — portable SIMD types for window and RMS reduction.
- `num-complex` 0.4.x — complex arithmetic for conformal fields and fractals.
