#!/bin/bash
echo "Testing right-click functionality"
echo "Press Shift+hint to right-click"
echo ""

# Kill any existing daemon
pkill -f hintsd

# Start daemon with logging
echo "Starting daemon with logging..."
RUST_LOG=info ./target/release/hintsd &
DAEMON_PID=$!
sleep 1

# Run hintsx
echo "Running hintsx - hold Shift and type a hint to test right-click"
./target/release/hintsx

# Kill daemon
kill $DAEMON_PID 2>/dev/null
