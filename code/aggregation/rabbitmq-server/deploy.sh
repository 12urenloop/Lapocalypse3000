#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is required"
  exit 1
fi

if ! command -v bun >/dev/null 2>&1; then
  echo "bun is required for control scripts"
  exit 1
fi

docker compose up -d
bun install

echo "RabbitMQ started"
echo "AMQP: amqp://localhost:5672"
echo "Management UI: http://localhost:15672"
