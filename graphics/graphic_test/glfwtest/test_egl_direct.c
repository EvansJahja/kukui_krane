/*
 * test_egl_direct.c - Direct EGL test without GLFW
 * 
 * Compile: gcc test_egl_direct.c -o test_egl_direct -lEGL -lwayland-client
 * Run:     EGL_LOG_LEVEL=debug ./test_egl_direct
 * 
 * This tests if EGL works directly with the Wayland display,
 * bypassing GLFW to isolate where the failure occurs.
 */

#define EGL_EGLEXT_PROTOTYPES
#include <EGL/egl.h>
#include <EGL/eglext.h>
#include <wayland-client.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(void) {
    printf("Testing direct EGL + Wayland...\n");
    
    // Connect to Wayland display
    struct wl_display* wl_dpy = wl_display_connect(NULL);
    if (!wl_dpy) {
        fprintf(stderr, "Failed to connect to Wayland display\n");
        return 1;
    }
    printf("Connected to Wayland display\n");
    
    // Get EGL display from Wayland display using eglGetDisplay (more portable)
    EGLDisplay egl_dpy = eglGetDisplay((EGLNativeDisplayType)wl_dpy);
    if (egl_dpy == EGL_NO_DISPLAY) {
        fprintf(stderr, "eglGetDisplay returned EGL_NO_DISPLAY\n");
    }
    
    if (egl_dpy == EGL_NO_DISPLAY) {
        fprintf(stderr, "Failed to get EGL display\n");
        wl_display_disconnect(wl_dpy);
        return 1;
    }
    printf("Got EGL display\n");
    
    // Initialize EGL
    EGLint major, minor;
    if (!eglInitialize(egl_dpy, &major, &minor)) {
        EGLint error = eglGetError();
        fprintf(stderr, "Failed to initialize EGL: error 0x%x\n", error);
        wl_display_disconnect(wl_dpy);
        return 1;
    }
    printf("EGL initialized: version %d.%d\n", major, minor);
    
    // Print EGL info
    printf("EGL_VENDOR: %s\n", eglQueryString(egl_dpy, EGL_VENDOR));
    printf("EGL_VERSION: %s\n", eglQueryString(egl_dpy, EGL_VERSION));
    printf("EGL_CLIENT_APIS: %s\n", eglQueryString(egl_dpy, EGL_CLIENT_APIS));
    
    // Check for OpenGL ES support
    const char* apis = eglQueryString(egl_dpy, EGL_CLIENT_APIS);
    if (strstr(apis, "OpenGL_ES")) {
        printf("OpenGL ES is supported\n");
    } else {
        printf("WARNING: OpenGL ES not in client APIs!\n");
    }
    
    // Try to choose a config
    EGLint config_attribs[] = {
        EGL_SURFACE_TYPE, EGL_WINDOW_BIT,
        EGL_RED_SIZE, 8,
        EGL_GREEN_SIZE, 8,
        EGL_BLUE_SIZE, 8,
        EGL_ALPHA_SIZE, 8,
        EGL_RENDERABLE_TYPE, EGL_OPENGL_ES3_BIT,
        EGL_NONE
    };
    
    EGLConfig config;
    EGLint num_configs;
    if (!eglChooseConfig(egl_dpy, config_attribs, &config, 1, &num_configs)) {
        EGLint error = eglGetError();
        fprintf(stderr, "eglChooseConfig failed: error 0x%x\n", error);
    } else if (num_configs == 0) {
        fprintf(stderr, "No matching EGL configs found\n");
        
        // Try with GLES2 instead
        printf("Trying with GLES2...\n");
        config_attribs[11] = EGL_OPENGL_ES2_BIT;
        if (!eglChooseConfig(egl_dpy, config_attribs, &config, 1, &num_configs)) {
            fprintf(stderr, "eglChooseConfig with GLES2 also failed\n");
        } else {
            printf("Found %d GLES2 configs\n", num_configs);
        }
    } else {
        printf("Found %d matching EGL configs\n", num_configs);
    }
    
    // Bind API
    if (!eglBindAPI(EGL_OPENGL_ES_API)) {
        fprintf(stderr, "Failed to bind OpenGL ES API\n");
    } else {
        printf("Bound OpenGL ES API\n");
    }
    
    // Create a context (without surface, just to test)
    EGLint context_attribs[] = {
        EGL_CONTEXT_MAJOR_VERSION, 3,
        EGL_CONTEXT_MINOR_VERSION, 0,
        EGL_NONE
    };
    
    EGLContext ctx = eglCreateContext(egl_dpy, config, EGL_NO_CONTEXT, context_attribs);
    if (ctx == EGL_NO_CONTEXT) {
        EGLint error = eglGetError();
        fprintf(stderr, "Failed to create EGL context: error 0x%x\n", error);
        
        // Error codes:
        // 0x3003 = EGL_BAD_ALLOC
        // 0x3004 = EGL_BAD_ATTRIBUTE
        // 0x3005 = EGL_BAD_CONFIG
        // 0x3006 = EGL_BAD_CONTEXT
        // 0x300D = EGL_BAD_MATCH
    } else {
        printf("SUCCESS: Created EGL context!\n");
        eglDestroyContext(egl_dpy, ctx);
    }
    
    // Cleanup
    eglTerminate(egl_dpy);
    wl_display_disconnect(wl_dpy);
    
    printf("Test complete\n");
    return 0;
}
