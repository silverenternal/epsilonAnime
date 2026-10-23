$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

$Binary = Join-Path $Root "epsilonAnime.exe"
if (-not (Test-Path $Binary)) {
    $Binary = Join-Path $Root "target/release/epsilonAnime.exe"
}
if (-not (Test-Path $Binary)) {
    cargo build --release
    $Binary = Join-Path $Root "target/release/epsilonAnime.exe"
}

$Python = Join-Path $Root "py/.venv/Scripts/python.exe"
if (-not (Test-Path $Python)) {
    Push-Location (Join-Path $Root "py")
    try {
        uv sync
    } finally {
        Pop-Location
    }
}

& $Binary @args
exit $LASTEXITCODE
