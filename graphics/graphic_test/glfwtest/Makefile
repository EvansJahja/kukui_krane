# GLFW/EGL Test Suite Makefile
# For MT8183 Kukui Krane Minecraft Wayland Investigation

CC = gcc
CFLAGS = -Wall -g

all: test_glfw_wayland test_glfw_opengl test_glfw_native test_glfw_x11 test_egl_direct test_egl_gbm

test_glfw_wayland: test_glfw_wayland.c
	$(CC) $(CFLAGS) $< -o $@ -lglfw

test_glfw_opengl: test_glfw_opengl.c
	$(CC) $(CFLAGS) $< -o $@ -lglfw

test_glfw_native: test_glfw_native.c
	$(CC) $(CFLAGS) $< -o $@ -lglfw

test_glfw_x11: test_glfw_x11.c
	$(CC) $(CFLAGS) $< -o $@ -lglfw

test_egl_direct: test_egl_direct.c
	$(CC) $(CFLAGS) $< -o $@ -lEGL -lwayland-client

test_egl_gbm: test_egl_gbm.c
	$(CC) $(CFLAGS) $< -o $@ -lEGL -lgbm

clean:
	rm -f test_glfw_wayland test_glfw_opengl test_glfw_native test_glfw_x11 test_egl_direct test_egl_gbm

.PHONY: all clean

# Run all tests
test: all
	@echo "=== Running GLFW Wayland Test ==="
	-EGL_LOG_LEVEL=debug ./test_glfw_wayland
	@echo ""
	@echo "=== Running GLFW OpenGL Test ==="
	-EGL_LOG_LEVEL=debug ./test_glfw_opengl
	@echo ""
	@echo "=== Running GLFW Native Test ==="
	-EGL_LOG_LEVEL=debug ./test_glfw_native
	@echo ""
	@echo "=== Running Direct EGL Test ==="
	-EGL_LOG_LEVEL=debug ./test_egl_direct
	@echo ""
	@echo "=== Running EGL GBM Test ==="
	-EGL_LOG_LEVEL=debug ./test_egl_gbm

.PHONY: test
