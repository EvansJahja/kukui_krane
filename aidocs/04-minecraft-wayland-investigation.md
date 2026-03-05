# Minecraft Wayland Investigation - MT8183 Kukui Krane

**Date:** 2026-03-03  
**Status:** SOLVED - Wrong EGL library was being loaded

---

## Goal

Run Minecraft 1.20.1 via PollyMC using native Wayland (not XWayland) to utilize GPU acceleration.

---

## Solution

### Root Cause
The system has `/usr/lib/mali-egl/` in `/etc/ld.so.conf`, which contains **Mali Utgard** proprietary EGL libraries (for Amlogic Meson). These are incompatible with the **Mali-G72 Bifrost** GPU on MT8183 which uses the **Panfrost** open-source driver.

When GLFW/Minecraft loads `libEGL.so.1`, it picks up `/usr/lib/mali-egl/libEGL.so.1` instead of Mesa's `/usr/lib/libEGL.so.1`.

### Quick Fix (Per-Launch)
Add to PollyMC instance wrapper command:
```bash
LD_PRELOAD=/usr/lib/libEGL.so.1
```

Or launch via terminal:
```bash
LD_PRELOAD=/usr/lib/libEGL.so.1 pollymc --launch "really vanilla 1.20.1"
```

### Permanent Fix (System-Wide)
Remove or disable the Mali EGL library path:
```bash
# Option 1: Remove the conf file that adds mali-egl
sudo rm /etc/ld.so.conf.d/mali-egl.conf  # or whichever file contains /usr/lib/mali-egl
sudo ldconfig

# Option 2: Uninstall the incompatible package
sudo pacman -R mali-utgard-meson-libgl-x11
```

### PollyMC Wrapper Limitation
**Note:** The PollyMC `WrapperCommand` setting doesn't properly propagate `LD_PRELOAD`/`LD_LIBRARY_PATH` to Java's native library loading (dlopen). The permanent fix (removing mali-egl from ld.so.conf) is recommended.

---

## How We Found the Root Cause

### 1. GLFW test programs failed with same error
Created minimal C test programs in [glfwtest/](glfwtest/) that reproduced the exact same EGL error, confirming it wasn't Minecraft/LWJGL specific.

### 2. Compared library linkage
```bash
# Working app (es2gears_wayland):
$ ldd /usr/bin/es2gears_wayland | grep EGL
libEGL.so.1 => /usr/lib/libEGL.so.1  # Mesa EGL ✅

# Failing test program:
$ ldd ./test_egl_direct | grep EGL
libEGL.so.1 => /usr/lib/mali-egl/libEGL.so.1  # Wrong Mali EGL ❌
```

### 3. Direct EGL test with Mali library fails
```bash
$ ./test_egl_direct
Failed to initialize EGL: error 0x3003  # EGL_BAD_ALLOC
```

### 4. Forcing Mesa EGL works
```bash
$ LD_PRELOAD=/usr/lib/libEGL.so.1 ./test_glfw_wayland
SUCCESS: GLFW + EGL + Wayland works!
```

### 5. Found the culprit in ld.so.conf
```bash
$ cat /etc/ld.so.conf.d/* | grep mali
/usr/lib/mali-egl

$ ldconfig -p | grep -i egl
libEGL.so.1 => /usr/lib/mali-egl/libEGL.so.1  # Listed first!
libEGL.so.1 => /usr/lib/libEGL.so.1
```

The package `mali-utgard-meson-libgl-x11` installs incompatible Mali Utgard libraries and adds them to the library search path, causing them to be loaded instead of Mesa.

---

## Setup

### Components
- **Launcher:** PollyMC (custom build)
  - Path: `/home/evans/polly/fn2006-PollyMC_-_2025-07-27_01-59-43/build/pollymc`
- **Instance:** "really vanilla 1.20.1"
  - Path: `/home/evans/.local/share/PollyMC/instances/really vanilla 1.20.1/`
- **Java:** OpenJDK 17 (`/usr/lib/jvm/java-17-openjdk/bin/java`)
- **GLFW:** `glfw-wayland-minecraft-cursorfix 3.4-6` (AUR package with Wayland + cursor fix patches)
  - Library: `/usr/lib/libglfw.so.3.4`

### Desktop Launcher
```
/home/evans/Desktop/really vanilla 1.20.1.desktop
```
```ini
[Desktop Entry]
Type=Application
Exec="/home/evans/polly/fn2006-PollyMC_-_2025-07-27_01-59-43/build/pollymc" '--launch' 'really vanilla 1.20.1'
Name=really vanilla 1.20.1
Icon=/home/evans/.local/share/PollyMC/instances/really vanilla 1.20.1/icon.png
```

### Instance Configuration
From `instance.cfg`:
```ini
CustomGLFWPath=/usr/lib/libglfw.so
JavaPath=/usr/lib/jvm/java-17-openjdk/bin/java
MaxMemAlloc=2579
```

Java launch includes:
```
-Dorg.lwjgl.glfw.libname=/usr/lib/libglfw.so
```

---

## Current Error

```
GLFW error 65542: EGL: Failed to initialize EGL: EGL failed to allocate resources for the requested operation.

Please make sure you have up-to-date drivers (see aka.ms/mcdriver for instructions).
```

GLFW error code 65542 = `GLFW_PLATFORM_ERROR` (0x10006)

---

## What We Verified

### 1. GLFW Has Wayland Support ✅
```bash
$ strings /usr/lib/libglfw.so.3.4 | grep -i wayland
glfwGetWaylandMonitor
glfwGetWaylandDisplay
glfwGetWaylandWindow
wayland
WAYLAND_DISPLAY
3.4.0 Wayland X11 GLX Null EGL OSMesa monotonic shared
!!! Patched GLFW from https://github.com/BoyOrigin/glfw-wayland
```

### 2. Minecraft Process Uses Wayland ✅
```bash
# File descriptors show wayland-cursor memfds
$ readlink /proc/<pid>/fd/* | grep wayland
/memfd:wayland-cursor (deleted)

# Environment variables set
WAYLAND_DISPLAY=wayland-0
GDK_BACKEND=wayland
SDL_VIDEODRIVER=wayland

# NOT in X11 window tree
$ xwininfo -root -tree | grep minecraft
(empty - not using X11)
```

### 3. EGL Works for Other Apps ✅
```bash
$ eglinfo | grep -A5 "GBM platform"
GBM platform:
EGL driver name: panfrost
OpenGL core profile renderer: Mali-G72 MC3 (Panfrost)
```

```bash
$ es2gears_wayland
EGL_VERSION = 1.5
~60 FPS (vsync limited)
```

---

## Root Cause Analysis

### Confirmed: Wrong EGL Library Loaded

The `mali-utgard-meson-libgl-x11` package installs proprietary Mali Utgard EGL libraries in `/usr/lib/mali-egl/` and adds this path to `/etc/ld.so.conf`. This causes the dynamic linker to prefer the incompatible Mali EGL over Mesa's EGL.

**Mali Utgard** is for older Mali-400/450 GPUs (used in Amlogic Meson SoCs), while the MT8183 has a **Mali-G72 Bifrost** GPU that requires either:
- Panfrost (open-source Mesa driver) ✅
- Mali Bifrost blob (proprietary ARM driver)

The Mali Utgard library returns `EGL_BAD_ALLOC` (0x3003) when it fails to initialize because it cannot communicate with the actual GPU hardware.

### Why es2gears_wayland Works

`es2gears_wayland` is explicitly linked against `/usr/lib/libEGL.so.1` (Mesa) at compile time, so it doesn't use the mali-egl path. Our test programs and GLFW dlopen the library at runtime, picking up the first match from ldconfig cache.

### Previous Hypotheses (Ruled Out)

**Hypothesis 1: Render Device Mismatch** - Not the cause. Both render nodes fail with Mali EGL, both work with Mesa EGL.

**Hypothesis 2: EGL Context Creation Issue** - Not the cause. Mesa EGL creates contexts successfully with Panfrost.

**Hypothesis 3: Memory/Resource Limits** - Not the cause. The "failed to allocate" error was from Mali Utgard driver, not actual resource exhaustion.

---

## Things to Try

### 1. Force EGL Device Selection
```bash
# Set render node explicitly
export MESA_LOADER_DRIVER_OVERRIDE=panfrost
export EGL_PLATFORM=wayland
```

### 2. Check EGL Debug Output
```bash
EGL_LOG_LEVEL=debug java ... (minecraft)
```

### 3. LWJGL EGL Configuration
Add to Java args:
```
-Dorg.lwjgl.egl.libname=/usr/lib/libEGL.so
-Dorg.lwjgl.opengl.libname=/usr/lib/libGL.so
```

### 4. Try Different GLFW Platform Hint
GLFW 3.4 supports platform hints:
```c
glfwInitHint(GLFW_PLATFORM, GLFW_PLATFORM_WAYLAND);
```

For Java/LWJGL, this might need environment variable:
```bash
export _JAVA_AWT_WM_NONREPARENTING=1
export GLFW_IM_MODULE=ibus  # or fcitx
```

### 5. Check for Resource Exhaustion Before Launch
```bash
# Check GPU memory
cat /sys/kernel/debug/dri/1/gem_names 2>/dev/null

# Check file descriptors
ulimit -n
ls /proc/$(pgrep java)/fd | wc -l
```

### 6. Test with Minimal GLFW Program ✅ DONE
Created test suite in [glfwtest/](glfwtest/) - see [glfwtest/README.md](glfwtest/README.md)

Build and run:
```bash
cd aidocs/glfwtest
make
make test
```

**Results:** All GLFW tests fail with same EGL error, confirming this is NOT Minecraft/LWJGL specific:
```
Testing GLFW + EGL + Wayland...
GLFW initialized successfully
Creating window with GLES 3.0 + EGL context...
GLFW error 65542: EGL: Failed to initialize EGL: EGL failed to allocate resources
```

### 7. Try Mesa Environment Variables
```bash
export MESA_GL_VERSION_OVERRIDE=3.3
export MESA_GLSL_VERSION_OVERRIDE=330
export MESA_GLES_VERSION_OVERRIDE=3.2
```

### 8. Check dmesg for GPU Errors
```bash
dmesg | grep -iE "panfrost|drm|gpu" | tail -20
```

Previous investigation showed some panfrost errors:
```
panfrost 13040000.gpu: js fault, js=0, status=DATA_INVALID_FAULT
panfrost 13040000.gpu: gpu sched timeout
```

---

## Instance Settings to Modify

In PollyMC, go to instance settings:
1. **Settings → Java** → Add JVM arguments
2. **Settings → Custom commands** → Add wrapper/pre-launch commands

Potential JVM arguments to add:
```
-Dorg.lwjgl.util.Debug=true
-Dorg.lwjgl.util.DebugLoader=true
```

Environment variables to set in instance:
```
EGL_LOG_LEVEL=debug
MESA_DEBUG=1
LIBGL_DEBUG=verbose
```

---

## Files to Check

- Minecraft logs: `/home/evans/.local/share/PollyMC/instances/really vanilla 1.20.1/.minecraft/logs/latest.log`
- Instance config: `/home/evans/.local/share/PollyMC/instances/really vanilla 1.20.1/instance.cfg`
- PollyMC global: `/home/evans/.local/share/PollyMC/pollymc.cfg`

---

## Related Documentation

- GLFW Wayland: https://www.glfw.org/docs/latest/compat.html#compat_wayland
- LWJGL Configuration: https://github.com/LWJGL/lwjgl3/wiki/Configuration
- Patched GLFW: https://github.com/BoyOrigin/glfw-wayland

---

## Next Steps

### To Fix Minecraft Launch:
1. **Apply permanent fix:** Remove mali-egl from library path
   ```bash
   sudo rm /etc/ld.so.conf.d/mali-egl.conf  # Find the actual file first
   sudo ldconfig
   ```
   Or uninstall: `sudo pacman -R mali-utgard-meson-libgl-x11`

2. **Test Minecraft launch** after fix is applied

3. **Verify GPU renderer in Minecraft** (F3 debug screen should show Mali-G72 Panfrost)

### Optional Further Investigation:
- Why is `mali-utgard-meson-libgl-x11` installed? May be a dependency of something else
- Test with LWJGL `-Dorg.lwjgl.egl.libname=/usr/lib/libEGL.so` as alternative to system fix

---

## Session Context

From GPU investigation (see 01-03 docs):
- KWin compositor uses GPU correctly (Mali-G72 Panfrost)
- Native Wayland EGL apps work
- XWayland/X11 apps fall back to llvmpipe (no DRI3)
- Electron apps use software rendering (ANGLE requirement)
