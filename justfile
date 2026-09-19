install *args:
    cargo fetch -- {{args}}

update *args:
    nix flake update
    cargo update -- {{args}}

format:
    treefmt .
    cargo fmt

test:
    cargo check
    cargo test
    cargo clippy --all-targets -- --deny warnings

build-dev *args:
    cargo build -- {{args}}

build-release *args:
    cargo build --release -- {{args}}

build-nix *args:
    nom build .#default -- {{args}}

run-dev *args:
    cargo run -- {{args}}

run-release *args:
    cargo run --release -- {{args}}

run-nix *args:
    nix run .#default -- {{args}} |& nom

clean *args:
    cargo clean -- {{args}}
