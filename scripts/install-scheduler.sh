#!/usr/bin/env bash
# Install a user-level systemd timer for peervault schedule-tick.
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: install-scheduler.sh --core <peervault> --config <jobs.json> --state <state.json> [--every 15min] [--credential <secret-file>] [--dry-run]
EOF
}

CORE="" CONFIG="" STATE="" EVERY="15min" CREDENTIAL="" DRY_RUN=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --core) CORE="${2:-}"; shift 2 ;;
        --config) CONFIG="${2:-}"; shift 2 ;;
        --state) STATE="${2:-}"; shift 2 ;;
        --every) EVERY="${2:-}"; shift 2 ;;
        --credential) CREDENTIAL="${2:-}"; shift 2 ;;
        --dry-run) DRY_RUN=1; shift ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown argument: $1" >&2; usage; exit 2 ;;
    esac
done

[[ -n "$CORE" && -n "$CONFIG" && -n "$STATE" ]] || { usage; exit 2; }
CORE="$(realpath "$CORE")"
CONFIG="$(realpath "$CONFIG")"
STATE="$(realpath -m "$STATE")"
if [[ -n "$CREDENTIAL" ]]; then
    CREDENTIAL="$(realpath "$CREDENTIAL")"
    [[ -f "$CREDENTIAL" ]] || { echo "credential file not found: $CREDENTIAL" >&2; exit 1; }
    mode="$(stat -c '%a' "$CREDENTIAL")"
    if (( 8#$mode & 8#077 )); then
        echo "credential file must not be group/other accessible (mode $mode)" >&2
        exit 1
    fi
fi
[[ -x "$CORE" ]] || { echo "core is not executable: $CORE" >&2; exit 1; }
[[ -f "$CONFIG" ]] || { echo "job config not found: $CONFIG" >&2; exit 1; }
[[ -d "$(dirname "$STATE")" ]] || { echo "state directory must exist" >&2; exit 1; }
if [[ "$EVERY" =~ [[:space:]] || "$CORE$CONFIG$STATE" =~ [[:space:]%] ]]; then
    echo "paths and interval must not contain whitespace or %" >&2
    exit 1
fi
if ! [[ "$EVERY" =~ ^[0-9]+(s|min|h|d)$ ]]; then
    echo "interval must look like 30s, 15min, 1h or 1d" >&2
    exit 1
fi

write_units() {
    local service="$1" timer="$2"
    cat >"$service" <<EOF
[Unit]
Description=PeerVault scheduled verification/scratch-restore tick
After=network.target

[Service]
Type=oneshot
ExecStart=$CORE schedule-tick --config $CONFIG --state $STATE
UMask=0077
NoNewPrivileges=yes
PrivateTmp=yes
EOF
    if [[ -n "$CREDENTIAL" ]]; then
        cat >>"$service" <<EOF
LoadCredential=peervault:$CREDENTIAL
Environment=PEERVAULT_PASSWORD_FILE=%d/peervault
EOF
    fi
    cat >"$timer" <<EOF
[Unit]
Description=Run PeerVault schedule-tick every $EVERY

[Timer]
OnBootSec=2min
OnUnitActiveSec=$EVERY
Persistent=true
Unit=peervault-tick.service

[Install]
WantedBy=timers.target
EOF
}

if [[ "$DRY_RUN" == "1" ]]; then
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    write_units "$work/peervault-tick.service" "$work/peervault-tick.timer"
    echo "# peervault-tick.service"
    cat "$work/peervault-tick.service"
    echo "# peervault-tick.timer"
    cat "$work/peervault-tick.timer"
    exit 0
fi

UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
mkdir -p "$UNIT_DIR"
umask 077
write_units "$UNIT_DIR/peervault-tick.service" "$UNIT_DIR/peervault-tick.timer"
systemctl --user daemon-reload
systemctl --user enable --now peervault-tick.timer
echo "Installed and started peervault-tick.timer every $EVERY."
echo "Scheduled secrets stay in the job config's password_file; no password is placed in the unit."
echo "Check: systemctl --user list-timers peervault-tick.timer"
