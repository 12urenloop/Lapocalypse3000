#!/usr/bin/env bash
set -e

if [[ $EUID -ne 0 ]]; then
    echo "Run as root: sudo bash $0"
    exit 1
fi

INSTALL_PATH="/usr/local/bin/ttyusb-logger.sh"
SERVICE_PATH="/etc/systemd/system/ttyusb-logger.service"
LOGDIR="/var/log/ttyusb-logger"
BAUD=115200

echo "Installing ttyUSB logger..."

# --- Create logger script ---
cat > "$INSTALL_PATH" << 'EOF'
#!/usr/bin/env bash

BAUD=115200
LOGDIR="/var/log/ttyusb-logger"
declare -A RUNNING_PIDS

mkdir -p "$LOGDIR"

log() {
    echo "[`date '+%Y-%m-%d %H:%M:%S'`] $1"
}

start_logger() {
    DEV="$1"

    if [[ -n "${RUNNING_PIDS[$DEV]}" ]]; then
        return
    fi

    if [[ ! -e "$DEV" ]]; then
        return
    fi

    log "Starting logger for $DEV"

    stty -F "$DEV" "$BAUD" raw -echo -echoe -echok

    SAFE_DEV_NAME=$(echo "$DEV" | tr '/' '_')
    DATESTR=$(date +"%Y%m%d-%H%M%S")
    UNIXTS=$(date +%s)

    OUTFILE="$LOGDIR/usbout-${SAFE_DEV_NAME}-${DATESTR}-${UNIXTS}.txt"

    cat "$DEV" >> "$OUTFILE" 2>/dev/null &
    RUNNING_PIDS[$DEV]=$!

    log "Logging $DEV to $OUTFILE"
}

stop_logger() {
    DEV="$1"

    if [[ -n "${RUNNING_PIDS[$DEV]}" ]]; then
        log "Stopping logger for $DEV"
        kill "${RUNNING_PIDS[$DEV]}" 2>/dev/null
        unset RUNNING_PIDS[$DEV]
    fi
}

scan_devices() {
    for DEV in /dev/ttyUSB*; do
        [[ -e "$DEV" ]] && start_logger "$DEV"
    done

    for DEV in "${!RUNNING_PIDS[@]}"; do
        [[ ! -e "$DEV" ]] && stop_logger "$DEV"
    done
}

log "ttyUSB logger started"

while true; do
    scan_devices
    sleep 5
done
EOF

chmod +x "$INSTALL_PATH"

# --- Create systemd service ---
cat > "$SERVICE_PATH" << EOF
[Unit]
Description=TTYUSB Logger Service
After=multi-user.target

[Service]
Type=simple
ExecStart=$INSTALL_PATH
Restart=always
RestartSec=3
User=root

[Install]
WantedBy=multi-user.target
EOF

# --- Setup log directory ---
mkdir -p "$LOGDIR"

# --- Enable service ---
systemctl daemon-reload
systemctl enable ttyusb-logger.service
systemctl restart ttyusb-logger.service

echo ""
echo "Installation complete."
echo "Service status:"
systemctl --no-pager status ttyusb-logger.service