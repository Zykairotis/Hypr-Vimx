#!/bin/bash

echo "=== Starting hints with full logging ==="
echo ""

# Kill existing daemon
pkill -f hintsd
sleep 1

# Start daemon with logging
echo "Starting hintsd with logging..."
RUST_LOG=info ./target/release/hintsd 2>&1 | tee /tmp/hintsd.log &
DAEMON_PID=$!
sleep 1

echo "Daemon running (PID: $DAEMON_PID)"
echo "Logs: /tmp/hintsd.log"
echo ""
echo "Now run: ./target/release/hintsx"
echo "Then type a hint to click"
echo ""
echo "Watch the terminal - you should see lines like:"
echo "  [INFO rust_hintsx::mouse] Mouse click: button=Left..."
echo ""
echo "To watch logs in real-time: tail -f /tmp/hintsd.log"
echo ""
echo "Press Ctrl+C to stop watching, daemon will continue running"
echo ""

tail -f /tmp/hintsd.log
