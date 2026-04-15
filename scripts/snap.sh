#!/usr/bin/env bash
# Grab a screenshot for iterative UI debugging.
#
# Picks a capture path based on environment:
#   1. WSLg (/proc/version mentions microsoft) → PowerShell screen capture.
#      WSLg renders Linux GUI apps via its own Weston compositor and projects
#      them straight into Windows, so Windows-side capture is the reliable
#      option — xdotool/import cannot see the Wayland-native Tauri window.
#   2. Otherwise, try `grim` (Wayland).
#   3. Otherwise, try `import -window <id>` with xdotool window-name match.
#   4. Otherwise, `import -window root`.
#
# Output: tmp/snap.png (gitignored).

set -euo pipefail

OUT_DIR="$(cd "$(dirname "$0")/.." && pwd)/tmp"
OUT="$OUT_DIR/snap.png"
mkdir -p "$OUT_DIR"

TITLE="tend"
FORCE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --wsl) FORCE="wsl"; shift ;;
    --grim) FORCE="grim"; shift ;;
    --x11) FORCE="x11"; shift ;;
    --name) TITLE="$2"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

is_wslg() {
  [[ -r /proc/version ]] && grep -qi microsoft /proc/version
}

capture_wsl() {
  # Copy the Windows primary screen to a PNG via PowerShell, then move it
  # into the repo. $env:TEMP resolves to the user's Windows temp dir, which
  # WSL can reach via /mnt/c/Users/…/AppData/Local/Temp/.
  local ps_out_win ps_out_wsl
  ps_out_win='$env:TEMP + "\tend-snap.png"'
  powershell.exe -NoProfile -Command "
    Add-Type -AssemblyName System.Windows.Forms
    Add-Type -AssemblyName System.Drawing
    \$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    \$bmp = New-Object System.Drawing.Bitmap \$b.Width, \$b.Height
    \$g = [System.Drawing.Graphics]::FromImage(\$bmp)
    \$g.CopyFromScreen(\$b.Location, [System.Drawing.Point]::Empty, \$b.Size)
    \$p = Join-Path \$env:TEMP 'tend-snap.png'
    \$bmp.Save(\$p, [System.Drawing.Imaging.ImageFormat]::Png)
    \$g.Dispose(); \$bmp.Dispose()
    Write-Output \$p
  " > /tmp/tend-snap-pspath.txt

  # PowerShell returns the Windows path; translate to WSL path.
  local win_path
  win_path="$(tr -d '\r\n' < /tmp/tend-snap-pspath.txt)"
  ps_out_wsl="$(wslpath -u "$win_path")"

  cp "$ps_out_wsl" "$OUT"
  rm -f /tmp/tend-snap-pspath.txt
  echo "→ $OUT (WSLg full screen)"
}

capture_grim() {
  grim "$OUT"
  echo "→ $OUT (wayland)"
}

capture_x11() {
  if command -v xdotool >/dev/null; then
    local wid
    wid="$(xdotool search --name "$TITLE" 2>/dev/null | tail -n1 || true)"
    if [[ -n "$wid" ]]; then
      xdotool windowactivate --sync "$wid" 2>/dev/null || true
      import -window "$wid" "$OUT"
      echo "→ $OUT (x11 window)"
      return
    fi
  fi
  import -window root "$OUT"
  echo "→ $OUT (x11 root)"
}

case "$FORCE" in
  wsl)  capture_wsl ;;
  grim) capture_grim ;;
  x11)  capture_x11 ;;
  *)
    if is_wslg && command -v powershell.exe >/dev/null; then
      capture_wsl
    elif command -v grim >/dev/null; then
      capture_grim
    elif command -v import >/dev/null; then
      capture_x11
    else
      echo "error: no capture tool available (install imagemagick, or grim, or run under WSLg)" >&2
      exit 1
    fi
    ;;
esac
