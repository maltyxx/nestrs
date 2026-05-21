# syntax=docker/dockerfile:1

FROM mcr.microsoft.com/devcontainers/rust:1-trixie AS dev

USER vscode

RUN rustup component add rust-analyzer llvm-tools-preview \
 && cargo install --locked cargo-binstall \
 && cargo binstall --no-confirm --locked \
      just bacon cargo-nextest cargo-llvm-cov
