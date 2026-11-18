#!/usr/bin/env bash
# Remove the user-level systemd timer; repository data and secrets are untouched.
set -euo pipefail
UNIT_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
systemctl --user disable --now peervault-tick.timer 2>/dev/null || true
rm -f "$UNIT_DIR/peervault-tick.timer" "$UNIT_DIR/peervault-tick.service"
systemctl --user daemon-reload
echo "Removed peervault-tick units. Repository data and credentials were not deleted."
