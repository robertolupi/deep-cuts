# Contributing to Deep Cuts

Thanks for your interest in contributing. This document covers the basics for getting started.

## Before You Start

- Check the [open issues](../../issues) to avoid duplicating work.
- For large changes (new features, architectural shifts), open an issue first to discuss the approach.
- Deep Cuts is licensed under AGPLv3. By contributing, you agree your code will be distributed under the same license.

## Development Setup

Follow the [Development & Build](README.md#-development--build) section in the README to get a working environment. Make sure tests pass before opening a PR:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
npm test
```

## Code Style

- **Rust**: standard `rustfmt` formatting (`cargo fmt`). Run `cargo clippy` and address warnings before submitting.
- **TypeScript/Svelte**: no enforced formatter currently — follow the style of surrounding code.
- **Comments**: only where the *why* is non-obvious. No restating what the code does.

## Submitting a Pull Request

1. Fork the repo and create a branch from `main`.
2. Make your changes with focused, logical commits.
3. Ensure all tests pass.
4. Open a PR against `main` with a clear description of what changed and why.

## Reporting Bugs

Open a GitHub issue with:
- Steps to reproduce
- Expected vs. actual behaviour
- OS version and relevant app/model versions

## Security Issues

Please do not report security vulnerabilities in public issues. Open a [GitHub Security Advisory](../../security/advisories/new) instead.
