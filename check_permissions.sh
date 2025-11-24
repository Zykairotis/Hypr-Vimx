#!/bin/bash

echo "=== Checking permissions for hints ==="
echo ""

echo "1. Current user:"
whoami
echo ""

echo "2. User groups:"
groups
echo ""

echo "3. /dev/uinput permissions:"
ls -la /dev/uinput
echo ""

echo "4. Check if user is in 'input' group:"
if groups | grep -q input; then
    echo "✓ User is in 'input' group"
else
    echo "✗ User is NOT in 'input' group"
    echo "   Run: sudo usermod -aG input $USER"
    echo "   Then logout and login again"
fi
echo ""

echo "5. Test uinput access:"
if [ -w /dev/uinput ]; then
    echo "✓ Can write to /dev/uinput"
else
    echo "✗ Cannot write to /dev/uinput"
    echo "   This is required for mouse clicks to work"
fi
echo ""

echo "6. Running processes:"
ps aux | grep hintsd | grep -v grep || echo "hintsd is not running"
echo ""

echo "=== Solutions ==="
echo "If clicks don't work, try:"
echo "1. Add user to input group: sudo usermod -aG input $USER"
echo "2. Logout and login again for group changes to take effect"
echo "3. Or run hintsd with sudo (not recommended for regular use):"
echo "   sudo ./target/release/hintsd &"
echo ""
echo "Alternative: Create a udev rule for /dev/uinput:"
echo "   echo 'KERNEL==\"uinput\", MODE=\"0666\"' | sudo tee /etc/udev/rules.d/99-uinput.rules"
echo "   sudo udevadm control --reload-rules && sudo udevadm trigger"
