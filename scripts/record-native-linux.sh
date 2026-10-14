#!/usr/bin/env bash
# Capture actual release-window input in an isolated desktop, never the active desktop.
set -euo pipefail
export OMP_THREAD_LIMIT=1
cd "$(dirname "$0")/.."
if [[ ${1:-} != --session ]]; then
  if [[ $# != 2 ]]; then echo 'Usage: record-native-linux.sh RELEASE_BINARY OUTPUT_DIRECTORY' >&2; exit 2; fi
  config=$(mktemp -d /tmp/peer-record-config-XXXXXX)
  trap 'rm -rf "$config"' EXIT
  export XDG_CONFIG_HOME="$config" GSETTINGS_BACKEND=memory
  xvfb-run -a -s '-screen 0 1440x900x24' dbus-run-session -- "$0" --session "$1" "$2"
  exit
fi
export XDG_RUNTIME_DIR=$(mktemp -d /tmp/peer-record-runtime-XXXXXX)
xfwm4 --compositor=off --replace >/tmp/peer-record-window-manager.log 2>&1 &
manager=$!
trap 'kill "$manager" 2>/dev/null || true; rm -rf "$XDG_RUNTIME_DIR"' EXIT
python3 scripts/record-native-demo.py "$2" "$3"
