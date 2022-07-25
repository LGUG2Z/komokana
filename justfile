set shell := ["cmd.exe", "/C"]
export RUST_BACKTRACE := "full"

clean:
    cargo clean

fmt:
    cargo +nightly fmt
    cargo +nightly clippy
    prettier --write README.md

install:
    cargo +stable install --path . --locked

run:
    cargo +stable run --bin komokana --locked

warn $RUST_LOG="warn":
    just run

info $RUST_LOG="info":
    just run

debug $RUST_LOG="debug":
    just run

trace $RUST_LOG="trace":
    just run