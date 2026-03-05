/*
 * test_glfw_x11.c - GLFW test forcing X11 platform
 * 
 * Compile: gcc test_glfw_x11.c -o test_glfw_x11 -lglfw
 * Run:     DISPLAY=:0 ./test_glfw_x11
 * 
 * Note: On Wayland sessions, this will use XWayland
 */

#define GLFW_EXPOSE_NATIVE_X11
#include <GLFW/glfw3.h>
#include <stdio.h>
#include <stdlib.h>

void error_callback(int error, const char* description) {
    fprintf(stderr, "GLFW error %d: %s\n", error, description);
}

int main(void) {
    glfwSetErrorCallback(error_callback);
    
    printf("Testing GLFW with X11 platform...\n");
    printf("DISPLAY=%s\n", getenv("DISPLAY") ?: "(not set)");
    printf("WAYLAND_DISPLAY=%s\n", getenv("WAYLAND_DISPLAY") ?: "(not set)");
    
    // Force X11 platform in GLFW 3.4+
    glfwInitHint(GLFW_PLATFORM, GLFW_PLATFORM_X11);
    
    if (!glfwInit()) {
        fprintf(stderr, "Failed to initialize GLFW with X11\n");
        return 1;
    }
    
    printf("GLFW initialized with X11\n");
    printf("GLFW version: %s\n", glfwGetVersionString());
    
    // Use GLX for X11
    glfwWindowHint(GLFW_CLIENT_API, GLFW_OPENGL_API);
    glfwWindowHint(GLFW_CONTEXT_CREATION_API, GLFW_NATIVE_CONTEXT_API);
    
    printf("Creating window with GLX context...\n");
    
    GLFWwindow* window = glfwCreateWindow(640, 480, "GLFW X11 Test", NULL, NULL);
    if (!window) {
        fprintf(stderr, "Failed to create GLFW window on X11\n");
        glfwTerminate();
        return 1;
    }
    
    printf("Window created successfully!\n");
    
    glfwMakeContextCurrent(window);
    printf("SUCCESS: X11/GLX context works!\n");
    
    glfwDestroyWindow(window);
    glfwTerminate();
    return 0;
}
