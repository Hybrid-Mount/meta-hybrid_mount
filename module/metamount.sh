
MODDIR="${0%/*}"
BASE_DIR="/data/adb/meta-hybrid"
LOG_FILE="$BASE_DIR/daemon.log"
mkdir -p "$BASE_DIR"
if [ -f "$LOG_FILE" ]; then
    rm "$LOG_FILE"
fi
log() {
    echo "[Wrapper] $1" >> "$LOG_FILE"
}
log "Starting Hybrid Mount..."
BINARY="$MODDIR/meta-hybrid"
if [ ! -f "$BINARY" ]; then
    log "ERROR: Binary not found at $BINARY"
    exit 1
fi

if [ -f "/data/adb/meta-hybrid/daemon.log" ]; then
  mv "/data/adb/meta-hybrid/daemon.log" "/data/adb/meta-hybrid/daemon.log.bak"
fi

chmod 755 "$BINARY"
"$BINARY" >> "$LOG_FILE" 2>&1
EXIT_CODE=$?
log "Hybrid Mount exited with code $EXIT_CODE"
if [ "$EXIT_CODE" = "0" ]; then
    /data/adb/ksud kernel notify-module-mounted
fi
exit $EXIT_CODE