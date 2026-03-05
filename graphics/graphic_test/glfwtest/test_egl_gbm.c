/*
 * test_egl_gbm.c - Test EGL with GBM (render node) directly
 * 
 * Compile: gcc test_egl_gbm.c -o test_egl_gbm -lEGL -lgbm
 * Run:     EGL_LOG_LEVEL=debug ./test_egl_gbm
 * 
 * This tests if EGL works with the GPU render node directly,
 * bypassing Wayland to isolate the issue.
 */

#define EGL_EGLEXT_PROTOTYPES
#include <EGL/egl.h>
#include <EGL/eglext.h>
#include <gbm.h>
#include <fcntl.h>
#include <unistd.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(void) {
    printf("Testing EGL with GBM render node...\n");
    
    // Try both render nodes
    const char* devices[] = {
        "/dev/dri/renderD128",  // mediatek-drm
        "/dev/dri/renderD129",  // panfrost
        NULL
    };
    
    for (int i = 0; devices[i]; i++) {
        printf("\n=== Testing %s ===\n", devices[i]);
        
        int fd = open(devices[i], O_RDWR);
        if (fd < 0) {
            perror("open");
            continue;
        }
        printf("Opened %s (fd=%d)\n", devices[i], fd);
        
        // Create GBM device
        struct gbm_device* gbm = gbm_create_device(fd);
        if (!gbm) {
            fprintf(stderr, "Failed to create GBM device\n");
            close(fd);
            continue;
        }
        printf("Created GBM device\n");
        
        // Get EGL display from GBM using eglGetDisplay (more portable)
        EGLDisplay egl_dpy = eglGetDisplay((EGLNativeDisplayType)gbm);
        
        if (egl_dpy == EGL_NO_DISPLAY) {
            fprintf(stderr, "Failed to get EGL display from GBM\n");
            gbm_device_destroy(gbm);
            close(fd);
            continue;
        }
        printf("Got EGL display from GBM\n");
        
        // Initialize EGL
        EGLint major, minor;
        if (!eglInitialize(egl_dpy, &major, &minor)) {
            EGLint error = eglGetError();
            fprintf(stderr, "Failed to initialize EGL: error 0x%x\n", error);
            gbm_device_destroy(gbm);
            close(fd);
            continue;
        }
        printf("EGL initialized: version %d.%d\n", major, minor);
        
        // Print info
        printf("EGL_VENDOR: %s\n", eglQueryString(egl_dpy, EGL_VENDOR));
        printf("EGL_VERSION: %s\n", eglQueryString(egl_dpy, EGL_VERSION));
        
        // List all configs
        EGLint num_all_configs;
        eglGetConfigs(egl_dpy, NULL, 0, &num_all_configs);
        printf("Total EGL configs available: %d\n", num_all_configs);
        
        // Try to find a usable config
        EGLint config_attribs[] = {
            EGL_SURFACE_TYPE, EGL_WINDOW_BIT,
            EGL_RENDERABLE_TYPE, EGL_OPENGL_ES3_BIT,
            EGL_NONE
        };
        
        EGLConfig config;
        EGLint num_configs;
        if (eglChooseConfig(egl_dpy, config_attribs, &config, 1, &num_configs) && num_configs > 0) {
            printf("Found %d configs with WINDOW_BIT + GLES3\n", num_configs);
            
            // Try to create context
            eglBindAPI(EGL_OPENGL_ES_API);
            EGLint ctx_attribs[] = {
                EGL_CONTEXT_MAJOR_VERSION, 3,
                EGL_NONE
            };
            EGLContext ctx = eglCreateContext(egl_dpy, config, EGL_NO_CONTEXT, ctx_attribs);
            if (ctx != EGL_NO_CONTEXT) {
                printf("SUCCESS: Created GLES3 context on %s!\n", devices[i]);
                eglDestroyContext(egl_dpy, ctx);
            } else {
                printf("Failed to create context: 0x%x\n", eglGetError());
            }
        } else {
            printf("No configs with WINDOW_BIT + GLES3\n");
            
            // Try without WINDOW_BIT (offscreen only)
            EGLint pbuffer_attribs[] = {
                EGL_SURFACE_TYPE, EGL_PBUFFER_BIT,
                EGL_RENDERABLE_TYPE, EGL_OPENGL_ES3_BIT,
                EGL_NONE
            };
            if (eglChooseConfig(egl_dpy, pbuffer_attribs, &config, 1, &num_configs) && num_configs > 0) {
                printf("Found %d configs with PBUFFER_BIT + GLES3 (offscreen only)\n", num_configs);
            }
        }
        
        eglTerminate(egl_dpy);
        gbm_device_destroy(gbm);
        close(fd);
    }
    
    printf("\nTest complete\n");
    return 0;
}
