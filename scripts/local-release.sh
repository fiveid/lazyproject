#!/usr/bin/env sh
set -eu

echo "==> Checking git working tree is clean..."
if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "ERROR: Must be run inside a git repository." >&2
  exit 1
fi

if [ -n "$(git status --porcelain)" ]; then
  echo "ERROR: Working tree has uncommitted changes. Commit or stash before local release." >&2
  git status --short >&2
  exit 1
fi

SHORT_SHA="$(git rev-parse --short HEAD)"
echo "==> Building gated local release (commit $SHORT_SHA)..."

echo "==> Running cargo fmt --check..."
cargo fmt --all -- --check

echo "==> Running cargo clippy..."
cargo clippy --all-targets --all-features -- -D warnings

echo "==> Running cargo test..."
cargo test --all-targets --all-features

if command -v cargo-audit >/dev/null 2>&1; then
  echo "==> Running cargo audit..."
  cargo audit
else
  echo "==> Skipping cargo audit (not installed; enforced in CI)."
fi

if command -v cargo-deny >/dev/null 2>&1; then
  echo "==> Running cargo deny..."
  cargo deny check
else
  echo "==> Skipping cargo deny (not installed; enforced in CI)."
fi

echo "==> Installing lazyproject (version+$SHORT_SHA) to ~/.cargo/bin..."
LAZYPROJECT_BUILD_SHA="$SHORT_SHA" cargo install --path . --locked --force

echo "==> Installed: $(lazyproject --version)"
