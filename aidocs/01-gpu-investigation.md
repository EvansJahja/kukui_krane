# GPU Acceleration Investigation - MT8183 Kukui Krane

**Date:** 2026-03-03  
**System:** MediaTek MT8183 Chromebook (Kukui Krane)  
**OS:** Arch Linux ARM with ChromeOS kernel 6.12.43

---

## Summary

**Status:** GPU IS available and working for KWin Wayland compositor, but X11/XWayland applications fall back to software rendering (llvmpipe).

---

## Hardware Configuration

### DRM Devices
- **card0 / renderD128:** `mediatek-drm` - Display controller (HDMI/DSI output)
- **card1 / renderD129:** `panfrost` - Mali-G72 MC3 GPU (actual rendering hardware)

The MT8183 has a split architecture where:
- Display controller (mediatek-drm) handles display output
- GPU (Mali-G72 via panfrost) handles 3D rendering

### Environment Variable
```
KWIN_DRM_DEVICES=/dev/dri/card1:/dev/dri/card0
```
KWin correctly uses panfrost (card1) as primary.

---

## Findings

### 1. KWin Wayland Compositor: ✅ WORKING
From `qdbus6 org.kde.KWin /KWin org.kde.KWin.supportInformation`:
```
Compositing Type: OpenGL
OpenGL renderer string: Mali-G72 MC3 (Panfrost)
OpenGL version string: 3.1 (Core Profile) Mesa 26.0.1-arch1.1
Driver: Panfrost
```

### 2. Native EGL/GBM (Wayland): ✅ WORKING
From `eglinfo`:
```
GBM platform:
EGL driver name: panfrost
OpenGL core profile renderer: Mali-G72 MC3 (Panfrost)

Wayland platform:
EGL driver name: mediatek
OpenGL core profile renderer: Mali-G72 MC3 (Panfrost)
```

### 3. X11/XWayland GLX: ❌ USING LLVMPIPE
From `glxinfo`:
```
screen 0 does not appear to be DRI3 capable
OpenGL renderer string: llvmpipe (LLVM 21.1.8, 128 bits)
Accelerated: no
```

The X11 platform EGL uses swrast:
```
X11 platform:
EGL driver name: swrast
OpenGL core profile renderer: llvmpipe (LLVM 21.1.8, 128 bits)
```

### 4. GPU Errors in dmesg
```
panfrost 13040000.gpu: js fault, js=0, status=DATA_INVALID_FAULT
panfrost 13040000.gpu: gpu sched timeout, js=0
panfrost 13040000.gpu: Soft-stop failed
```
These errors indicate panfrost driver instability, but KWin still renders correctly.

---

## Root Cause Analysis

### Why XWayland Glamor Fails

XWayland needs to provide hardware-accelerated GLX via the "glamor" module, which requires:
1. GBM (Generic Buffer Management) support
2. Either `wl_drm` or `zwp_linux_dmabuf` Wayland protocols

Error strings found in Xwayland binary:
```
xwayland glamor: failed to setup GBM backend, falling back to sw accel
glamor: 'wl_drm' not supported and linux-dmabuf v4 not supported
Xwayland glamor: GBM Wayland interfaces not available
```

The compositor provides `zwp_linux_dmabuf_v1` version 5, but glamor initialization is still failing.

### Architecture Issue
On systems with separate display controller (mediatek-drm) and rendering GPU (panfrost):
- X11 display is served by XWayland
- XWayland receives EGL context from KWin
- For GLX, XWayland needs DRI3 which requires matching render capabilities
- mediatek-drm (card0) doesn't support DRI3 rendering, only display
- panfrost (card1) supports rendering but isn't being used for X11

---

## Wayland Protocol Support

From `wayland-info`:
- `zwp_linux_dmabuf_v1` version 5 ✅
- `wp_drm_lease_device_v1` version 1 ✅

Protocols are available, but XWayland glamor still can't initialize.

---

## Next Steps to Investigate

1. **Check XWayland glamor configuration**
   - Force `-glamor gl` or `-glamor es` 
   - Check if KWin passes glamor options to XWayland

2. **Check GBM render device selection**
   - XWayland may be trying to use card0 (mediatek-drm) instead of card1 (panfrost) for GBM
   - Look for `GBM_DEVICE` environment variable support

3. **Test native Wayland apps**
   - Native Wayland applications should use GPU correctly
   - Only X11/XWayland apps are affected

4. **Check Mesa/panfrost bugs**
   - GPU faults may be related to glamor issues
   - Check for known issues with panfrost + XWayland glamor

5. **Workarounds**
   - Use Qt/GTK apps in Wayland mode (QT_QPA_PLATFORM=wayland, GDK_BACKEND=wayland)
   - Minimize X11 app usage

---

## Commands Used

```bash
# Check DRM devices
ls -la /dev/dri/
cat /sys/class/drm/card*/device/uevent

# Check GPU driver status  
dmesg | grep -iE "panfrost|drm|gpu|mali"
lsmod | grep -E "drm|gpu|mali|panfrost"

# Check EGL platforms
eglinfo | grep -E "platform:|renderer|driver"

# Check GLX/X11 rendering
glxinfo | grep -E "renderer|Device|Accelerated"
LIBGL_DEBUG=verbose glxinfo 2>&1 | grep -iE "direct|dri"

# Check KWin status
qdbus6 org.kde.KWin /KWin org.kde.KWin.supportInformation | grep -A30 "OpenGL"

# Check Wayland protocols
wayland-info | grep -iE "drm|dmabuf"
```
