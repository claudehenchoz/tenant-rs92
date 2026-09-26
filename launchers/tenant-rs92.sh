#!/usr/bin/env sh
# Starts the TENANT RS-92 standalone (the app uses the device's own sample rate and the device's own buffer size) (JACK if running, else ALSA).
# Set RS92_MIDI to a MIDI input name (run with --midi-input "" to list them).
DIR="$(cd "$(dirname "$0")" && pwd)"
if [ -n "$RS92_MIDI" ]; then
  exec "$DIR/tenant-rs92" --midi-input "$RS92_MIDI" "$@"
else
  exec "$DIR/tenant-rs92" "$@"
fi
