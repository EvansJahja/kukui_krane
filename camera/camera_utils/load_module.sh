#!/bin/bash
# Load/Reload MT8183 Camera ISP Module
#
# This script unloads and reloads the mtk-cam-isp kernel module.
# Run this after each reboot or when the camera stops working.
#
# Usage: sudo ./load_module.sh

set -e

MODULE_NAME="mtk_cam_isp"
MODULE_PATH="/home/evans/linux-root/linux/chromeos-6.12/drivers/media/platform/mediatek/isp/isp_50/cam/mtk-cam-isp.ko"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if running as root
if [[ $EUID -ne 0 ]]; then
    error "This script must be run as root (use sudo)"
    exit 1
fi

# Check if module file exists
if [[ ! -f "$MODULE_PATH" ]]; then
    error "Module not found: $MODULE_PATH"
    echo "Build it with: make -j8 M=drivers/media/platform/mediatek/isp/isp_50/cam modules"
    exit 1
fi

echo "=== MT8183 Camera Module Loader ==="
echo ""

# Unload existing module (ignore errors if not loaded)
info "Unloading existing module (if loaded)..."
rmmod "$MODULE_NAME" 2>/dev/null && info "  Module unloaded" || warn "  Module was not loaded"

# Load the module
info "Loading module from: $MODULE_PATH"
if insmod "$MODULE_PATH"; then
    info "Module loaded successfully!"
else
    error "Failed to load module"
    exit 1
fi

# Verify
echo ""
info "Verifying..."
if lsmod | grep -q "$MODULE_NAME"; then
    info "Module is active:"
    lsmod | grep "$MODULE_NAME"
else
    error "Module not found in lsmod output"
    exit 1
fi

# Check for video devices
echo ""
info "Camera devices:"
v4l2-ctl --list-devices 2>/dev/null | grep -A5 "mtk-cam-p1" || warn "No mtk-cam-p1 devices found"

echo ""
info "Ready! Run ./capture.py to take a photo"
