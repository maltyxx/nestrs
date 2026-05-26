# syntax=docker/dockerfile:1

FROM mcr.microsoft.com/devcontainers/rust:1-trixie AS dev

USER vscode

RUN rustup toolchain install stable \
      --component rustfmt --component clippy --component rust-analyzer --component llvm-tools-preview \
 && rustup default stable \
 && cargo install --locked cargo-binstall \
 && cargo binstall --no-confirm --locked \
      just bacon cargo-nextest cargo-llvm-cov cargo-audit cargo-outdated
