#!/bin/bash

echo "Starting ydotoold with proper permissions..."
echo "This requires sudo password to access /dev/uinput"
echo ""

# Kill any existing ydotoold
sudo pkill -9 ydotoold 2>/dev/null
pkill -9 ydotoold 2>/dev/null

# Remove old sockets
rm -f $HOME/.ydotool_socket 2>/dev/null
sudo rm -f /tmp/.ydotool_socket 2>/dev/null

# Start ydotoold as root but with socket owned by user
echo "Starting ydotoold daemon..."
sudo ydotoold --socket-path="$HOME/.ydotool_socket" --socket-own="$(id -u):$(id -g)" &

# Wait for it to start
echo "Waiting for daemon to start..."
sleep 3

# Test it
echo "Testing ydotool click..."
YDOTOOL_SOCKET="$HOME/.ydotool_socket" ydotool click 0xC0

echo ""
echo "If you saw 'c0 110' above, ydotool is NOT working."
echo "If nothing printed or the mouse clicked, it's working!"
echo ""
echo "Now run: ./target/release/hintsx"
