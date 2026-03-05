# GPU Acceleration Fix Research - XWayland DRI3

**Date:** 2026-03-03  
**Session:** Continued investigation

---

## Root Cause Identified

### The Problem Chain
1. **XWayland glamor fails to initialize** → Falls back to software rendering
2. **Without glamor, DRI3 extension is not available** in XWayland
3. **Without DRI3, X11 clients cannot use hardware-accelerated OpenGL**
4. **They fall back to llvmpipe** (CPU software rendering)

### Evidence
```bash
# XWayland extensions (no DRI2, no DRI3!)
$ xdpyinfo | grep extensions -A30
number of extensions:    24
    BIG-REQUESTS
    ...
    GLX          # Present but software only
    ...
    XWAYLAND
# NO DRI2, NO DRI3!

# EGL X11 error
$ es2gears_x11
libEGL warning: DRI3 error: Could not get DRI3 device
libEGL warning: Ensure your X server supports DRI3 to get accelerated rendering

# GLX shows software
$ glxinfo | grep renderer
OpenGL renderer string: llvmpipe (LLVM 21.1.8, 128 bits)
```

### Why Glamor Fails

XWayland outputs these error paths (from binary strings):
```
xwayland glamor: failed to setup GBM backend, falling back to sw accel
glamor: 'wl_drm' not supported and linux-dmabuf v4 not supported
Xwayland glamor: GBM Wayland interfaces not available
```

**However**, the compositor DOES provide `zwp_linux_dmabuf_v1` version 5.

The likely issue is **device mismatch** in the GBM/EGL initialization:
- KWin renders on `card1` (panfrost GPU)
- XWayland gets display connection to `card0` (mediatek-drm display controller)
- Glamor can't create compatible GBM buffers between the two devices

---

## Attempted Solutions (Not Working)

### 1. Environment Variables for DRI Selection
```bash
DRI_PRIME=1 glxinfo               # Still llvmpipe
MESA_LOADER_DRIVER_OVERRIDE=panfrost glxinfo  # Still llvmpipe
GBM_BACKEND=panfrost glxinfo      # Still llvmpipe
```
These don't work because the problem is XWayland/glamor not GLX client-side.

### 2. XWayland Launch Parameters
Current XWayland launch (by KWin):
```
/usr/bin/Xwayland :0 -auth /run/user/1000/xauth_greQcD -listenfd 8 -listenfd 9 -displayfd 70 -wm 72 -rootless -enable-ei-portal
```
KWin doesn't pass `-glamor` flag; XWayland tries auto-detection but fails.

---

## Potential Fixes to Try

### Fix 1: Force XWayland glamor device via udev

Create a udev rule to set the default render node to panfrost:
```bash
# /etc/udev/rules.d/61-mmc-render.rules
SUBSYSTEM=="drm", KERNEL=="renderD129", TAG+="uaccess", SYMLINK+="dri/default_render_node"
```

### Fix 2: Set MESA_GL_VERSION_OVERRIDE for panfrost

The panfrost driver supports OpenGL ES 3.2 and OpenGL 3.1. Try forcing:
```bash
export MESA_GL_VERSION_OVERRIDE=3.1
export MESA_GLES_VERSION_OVERRIDE=3.2
```

### Fix 3: Rebuild XWayland with explicit GBM device support

If XWayland has patches for multi-GPU, enable them.

### Fix 4: KWIN_DRM_DEVICES ordering

Already set: `KWIN_DRM_DEVICES=/dev/dri/card1:/dev/dri/card0`
KWin uses panfrost as primary, but this doesn't affect XWayland's internal glamor.

### Fix 5: Disable XWayland (Use Native Wayland Only)

For apps that support Wayland natively:
```bash
# Qt apps
export QT_QPA_PLATFORM=wayland

# GTK apps  
export GDK_BACKEND=wayland

# Force specific app
QT_QPA_PLATFORM=wayland app_name
```

This won't help for X11-only apps but fixes Qt/GTK apps.

### Fix 6: Check for KWin bug / upstream fix

Search KDE bugzilla for:
- XWayland glamor multi-GPU
- mediatek panfrost XWayland
- DRI3 XWayland ARM

---

## Current Status Summary

| Component | Status | Renderer |
|-----------|--------|----------|
| KWin Compositor | ✅ GPU | Mali-G72 MC3 (Panfrost) |
| Native Wayland EGL | ✅ GPU | Mali-G72 MC3 (Panfrost) |
| XWayland Glamor | ❌ Failed | - |
| DRI3 Extension | ❌ Not available | - |
| X11 GLX | ❌ Software | llvmpipe |
| X11 EGL | ❌ Software | llvmpipe (no DRI3) |

---

## Recommended Workaround

Until upstream fixes the glamor/multi-GPU issue, use **native Wayland mode** for applications:

Create `~/.config/plasma-workspace/env/wayland-apps.sh`:
```bash
#!/bin/bash
# Force Qt/GTK apps to use native Wayland
export QT_QPA_PLATFORM=wayland
export GDK_BACKEND=wayland
export MOZ_ENABLE_WAYLAND=1
export ELECTRON_OZONE_PLATFORM_HINT=wayland
```

This ensures Qt, GTK, Firefox, and Electron apps use native Wayland (with GPU) instead of XWayland (software).
