#!/bin/bash

echo "=== Testing mouse click functionality ==="
echo ""
echo "This will test left and right clicks"
echo ""

# Test sending a click command directly to the daemon
echo "Testing direct click via Unix socket..."

# Create a simple Python script to send a click request
cat > /tmp/test_click.py << 'EOF'
import socket
import struct
import json

# Simple test to send a click request to hintsd
sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
sock.connect('/tmp/hints.socket')

# Create a click request (this is a simplified version)
# Format: Click { x: 100, y: 100, button: 0, button_states: [1, 0], repeat: 1, absolute: true }
request = {
    "Click": {
        "x": 500,
        "y": 500,
        "button": 0,  # 0 = left, 2 = right
        "button_states": [1, 0],  # down, up
        "repeat": 1,
        "absolute": True
    }
}

# This would need proper bincode serialization, but for testing...
print("Would send click request to daemon")
print("Request:", json.dumps(request, indent=2))
sock.close()
EOF

python3 /tmp/test_click.py 2>/dev/null || echo "Direct socket test needs proper bincode serialization"

echo ""
echo "Now testing with actual hintsx..."
echo "Watch the terminal output and mouse pointer"
echo ""
echo "1. Run: ./target/release/hintsx"
echo "2. Type a hint to LEFT click"
echo "3. Hold Shift and type a hint to RIGHT click"
echo "4. Check /tmp/hintsd.log for 'Mouse click:' messages"
echo ""
echo "Current log tail:"
tail -5 /tmp/hintsd.log
