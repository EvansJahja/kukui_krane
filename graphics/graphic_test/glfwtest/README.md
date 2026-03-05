# GLFW/EGL Test Suite

Test programs for investigating Minecraft Wayland EGL initialization failure on MT8183 Kukui Krane.

## Build

```bash
make
```

## Tests

| Test | Purpose | Expected Result |
|------|---------|-----------------|
| `test_glfw_wayland` | GLFW + EGL + GLES3 on Wayland | Shows if GLFW EGL init works |
| `test_glfw_opengl` | GLFW + EGL + Desktop GL 3.3 | Tests desktop GL compatibility |
| `test_glfw_native` | GLFW with auto-detect context | Tests native context API |
| `test_glfw_x11` | Force X11/XWayland platform | Tests if X11 path works |
| `test_egl_direct` | Direct EGL + Wayland (no GLFW) | Isolates EGL from GLFW |
| `test_egl_gbm` | EGL with GBM render nodes | Tests GPU render nodes directly |

## Run All Tests

```bash
make test
```

Or individually with debug:
```bash
EGL_LOG_LEVEL=debug ./test_glfw_wayland
EGL_LOG_LEVEL=debug ./test_egl_direct
```

## Expected Output (Working System)

```
Testing GLFW + EGL + Wayland...
WAYLAND_DISPLAY=wayland-0
GLFW initialized successfully
Creating window with GLES 3.0 + EGL context...
Window created successfully!
SUCCESS: GLFW + EGL + Wayland works!
```

## Current Failure (MT8183)

```
GLFW error 65542: EGL: Failed to initialize EGL: EGL failed to allocate resources for the requested operation.
```

This indicates EGL initialization fails during `glfwCreateWindow()` despite `glfwInit()` succeeding.

## Related Files

- [04-minecraft-wayland-investigation.md](../04-minecraft-wayland-investigation.md)
- [03-final-summary.md](../03-final-summary.md)
