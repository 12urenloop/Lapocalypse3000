#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

if ! command -v bun >/dev/null 2>&1; then
  echo "bun is required. Install from https://bun.sh"
  exit 1
fi

if [[ ! -f .env ]]; then
  cp .env.example .env
  echo "created .env from .env.example"
fi

bun install
bun run start
