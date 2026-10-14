#!/usr/bin/env bash
# Disposable real desktop tests. Requires Xvfb, xfwm4, wmctrl, xdotool, xclip,
# ImageMagick, Tesseract, dbus-run-session and (desktop scenario) GTK portals.
set -euo pipefail
export OMP_THREAD_LIMIT=1
cd "$(dirname "$0")/.."
if [[ $(uname -s) != Linux ]]; then echo 'These window tests require Linux/X11' >&2; exit 1; fi
if [[ ${1:-} != --session ]]; then
  scenario=${1:-smoke}
  case "$scenario" in smoke|desktop|navigation|errors|tree|transition) ;; *) echo 'Use smoke, desktop, navigation, errors, tree or transition' >&2; exit 2;; esac
  output=${2:-test-results/native-$scenario}
  config=$(mktemp -d /tmp/peer-test-config-XXXXXX)
  trap 'rm -rf "$config"' EXIT
  export XDG_CONFIG_HOME="$config" GSETTINGS_BACKEND=memory
  exec_command=(xvfb-run -a -s '-screen 0 1440x900x24' dbus-run-session -- "$0" --session "$scenario" "$output")
  "${exec_command[@]}"
  exit
fi
scenario=$2
output=$3
mkdir -p "$output"
export XDG_RUNTIME_DIR=$(mktemp -d /tmp/peer-test-runtime-XXXXXX)
children=()
cleanup() {
  for child in "${children[@]}"; do kill "$child" 2>/dev/null || true; done
  wait 2>/dev/null || true
  rm -rf "$XDG_RUNTIME_DIR"
}
trap cleanup EXIT
xfwm4 --compositor=off --replace > "$output/window-manager.log" 2>&1 &
children+=("$!")
if [[ $scenario == desktop ]]; then
  prefix=${PEER_PORTAL_PREFIX:-/usr}
  export XDG_CURRENT_DESKTOP=GNOME
  export XDG_DATA_DIRS="$prefix/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
  export XDG_DESKTOP_PORTAL_DIR="$prefix/share/xdg-desktop-portal/portals"
  export GTK_DATA_PREFIX="$prefix"
  "$prefix/libexec/xdg-desktop-portal-gtk" > "$output/portal-gtk.log" 2>&1 &
  children+=("$!")
  "$prefix/libexec/xdg-desktop-portal" > "$output/portal.log" 2>&1 &
  children+=("$!")
fi
if [[ $scenario == smoke ]]; then
  python3 tests/native_smoke.py "${PEER_NATIVE_BINARY:-target/debug/peer}" "$output"
elif [[ $scenario == transition ]]; then
  python3 tests/native_tree_transition.py "${PEER_NATIVE_BINARY:-target/debug/peer}" "$output"
elif [[ $scenario == tree ]]; then
  python3 tests/native_tree.py "${PEER_NATIVE_BINARY:-target/debug/peer}" "$output"
else
  PEER_NATIVE_SCENARIO="$scenario" python3 tests/native_desktop.py "${PEER_NATIVE_BINARY:-target/debug/peer}" "$output"
fi
