# Final Summary - GPU Acceleration on MT8183 Kukui Krane

**Date:** 2026-03-03  
**Status:** PARTIALLY RESOLVED

---

## Conclusion

### Is GPU being utilized?

**YES, for native Wayland applications**
**NO, for X11/XWayland applications**

| Rendering Path | GPU Used? | Renderer | Notes |
|----------------|-----------|----------|-------|
| KWin Compositor | ✅ Yes | Mali-G72 MC3 (Panfrost) | Desktop effects work |
| Native Wayland EGL | ✅ Yes | Mali-G72 MC3 (Panfrost) | ~60 FPS (vsync) |
| Qt/GTK Wayland | ✅ Yes | Mali-G72 MC3 (Panfrost) | Works great |
| Electron/Chromium | ❌ No | llvmpipe (CPU) | ANGLE requirement |
| X11/XWayland GLX | ❌ No | llvmpipe (CPU) | No DRI3 |

---

## Root Cause

**XWayland glamor fails to initialize** on systems with split display/GPU architecture (mediatek-drm + panfrost). Without glamor:
- DRI3 extension is unavailable
- X11 clients cannot access GPU hardware
- Fall back to llvmpipe software rendering

The KWin Wayland compositor correctly uses the GPU, but it cannot provide accelerated XWayland due to incompatible buffer sharing between:
- card0 (mediatek-drm): Display controller only
- card1 (panfrost): GPU render device

---

## Applied Fix

Created `~/.config/plasma-workspace/env/wayland-apps.sh`:
```bash
#!/bin/bash
export QT_QPA_PLATFORM=wayland
export GDK_BACKEND=wayland
export MOZ_ENABLE_WAYLAND=1
export ELECTRON_OZONE_PLATFORM_HINT=wayland
export SDL_VIDEODRIVER=wayland
export CLUTTER_BACKEND=wayland
```

**After logout/login**, Qt, GTK, Firefox, Electron, and SDL apps will use native Wayland and GPU acceleration.

---

## What Still Uses Software Rendering

### Electron/Chromium Apps (VSCodium, etc.)
Even though they run on native Wayland, **Electron/Chromium apps use software rendering** on this device.

**Root Cause:** Modern Chromium requires **ANGLE** (Almost Native Graphics Layer Engine) for GPU acceleration. It only allows these GL implementations:
```
(gl=egl-angle,angle=opengl)
(gl=egl-angle,angle=opengles)
(gl=egl-angle,angle=vulkan)
(gl=egl-angle,angle=swiftshader)
```

- Native EGL/GLES (`egl-gles2`) is **not allowed**
- ANGLE-vulkan requires working Vulkan (panfrost Vulkan is experimental)
- ANGLE-opengles/opengl fails to initialize on panfrost

**Evidence:**
```
$ codium --status
GPU Status: gpu_compositing: disabled_software
            webgl: unavailable_software
```

**Log error:**
```
Requested GL implementation (gl=egl-gles2,angle=none) not found in allowed implementations
```

### Legacy X11-only Apps
- Some older GTK2 apps
- Wine/Proton games
- Some proprietary apps

---

## Future Potential Fixes

1. **Upstream Mesa/XWayland fix**: Multi-GPU glamor support for ARM SoCs with split display/render
2. **Kernel modification** (not available in this session): Device tree modifications to present unified DRM device
3. **Alternative compositor**: Test with Weston to see if glamor works there

### For Electron/Chromium GPU Acceleration:
4. **Better panfrost Vulkan**: If `PAN_MESA_DEBUG=gl3` or Vulkan matures, ANGLE-vulkan could work
5. **Custom Electron build**: Build Electron without ANGLE requirement (allows native EGL/GLES)
6. **Wait for upstream**: Chromium may eventually support native GLES on ARM again

---

## Commands for Verification

```bash
# Check KWin renderer (should show Mali-G72 Panfrost)
qdbus6 org.kde.KWin /KWin org.kde.KWin.supportInformation | grep -A5 "OpenGL"

# Check X11 renderer (will show llvmpipe until upstream fix)
glxinfo | grep "OpenGL renderer"

# Check Wayland EGL renderer (should show Panfrost)
eglinfo | grep -A2 "GBM platform"

# Test Wayland GPU (should be smooth)
es2gears_wayland

# Test X11 software (will be CPU-bound)
glxgears
```

---

## Session Files Created

1. [01-gpu-investigation.md](01-gpu-investigation.md) - Initial investigation and findings
2. [02-xwayland-dri3-fix-research.md](02-xwayland-dri3-fix-research.md) - Detailed root cause analysis
3. [03-final-summary.md](03-final-summary.md) - This file
4. `~/.config/plasma-workspace/env/wayland-apps.sh` - Applied workaround

---

## Next Session Actions

If you need to investigate further:
1. Check if there's a KDE Plasma 6.7+ update with XWayland glamor fixes
2. Test with `KWIN_XWAYLAND_DEBUG=1` for more verbose XWayland logs
3. Consider reporting bug to KDE/Mesa with this analysis
