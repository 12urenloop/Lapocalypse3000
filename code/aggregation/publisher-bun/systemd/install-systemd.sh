#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORKDIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
SERVICE_NAME="publisher-bun"
SERVICE_TEMPLATE="${SCRIPT_DIR}/publisher-bun.service.template"
LOGROTATE_TEMPLATE="${SCRIPT_DIR}/publisher-bun.logrotate.template"

SERVICE_USER="${1:-${SUDO_USER:-$USER}}"
SERVICE_GROUP="$(id -gn "$SERVICE_USER")"
SERVICE_HOME="$(getent passwd "$SERVICE_USER" | cut -d: -f6)"

if [[ -z "$SERVICE_HOME" ]]; then
  echo "Could not determine home directory for user: ${SERVICE_USER}"
  exit 1
fi

if [[ ! -f "${WORKDIR}/.env" ]]; then
  echo "Missing ${WORKDIR}/.env"
  echo "Create it from ${WORKDIR}/.env.example first."
  exit 1
fi

SERVICE_RENDERED="$(mktemp)"
LOGROTATE_RENDERED="$(mktemp)"
trap 'rm -f "$SERVICE_RENDERED" "$LOGROTATE_RENDERED"' EXIT

sed \
  -e "s|__SERVICE_USER__|${SERVICE_USER}|g" \
  -e "s|__WORKDIR__|${WORKDIR}|g" \
  -e "s|__SERVICE_HOME__|${SERVICE_HOME}|g" \
  "$SERVICE_TEMPLATE" > "$SERVICE_RENDERED"

sed \
  -e "s|__SERVICE_USER__|${SERVICE_USER}|g" \
  -e "s|__SERVICE_GROUP__|${SERVICE_GROUP}|g" \
  "$LOGROTATE_TEMPLATE" > "$LOGROTATE_RENDERED"

sudo install -d -m 0755 -o "$SERVICE_USER" -g "$SERVICE_GROUP" /var/log/publisher-bun
sudo touch /var/log/publisher-bun/publisher-bun.log
sudo chown "$SERVICE_USER:$SERVICE_GROUP" /var/log/publisher-bun/publisher-bun.log

sudo install -m 0644 "$SERVICE_RENDERED" /etc/systemd/system/${SERVICE_NAME}.service
sudo install -m 0644 "$LOGROTATE_RENDERED" /etc/logrotate.d/${SERVICE_NAME}

sudo systemctl daemon-reload
sudo systemctl enable --now ${SERVICE_NAME}.service

sudo systemctl status ${SERVICE_NAME}.service --no-pager || true

echo
echo "Installed and started ${SERVICE_NAME}.service"
echo "Logs: /var/log/publisher-bun/publisher-bun.log"
echo "Force logrotate test: sudo logrotate -f /etc/logrotate.d/${SERVICE_NAME}"
