/*
 * test_glfw_wayland.c - Minimal GLFW + EGL + Wayland test
 * 
 * Compile: gcc test_glfw_wayland.c -o test_glfw_wayland -lglfw
 * Run:     EGL_LOG_LEVEL=debug ./test_glfw_wayland
 */

#include <GLFW/glfw3.h>
#include <stdio.h>
#include <stdlib.h>

void error_callback(int error, const char* description) {
    fprintf(stderr, "GLFW error %d: %s\n", error, description);
}

int main(void) {
    glfwSetErrorCallback(error_callback);
    
    printf("Testing GLFW + EGL + Wayland...\n");
    printf("WAYLAND_DISPLAY=%s\n", getenv("WAYLAND_DISPLAY") ?: "(not set)");
    
    if (!glfwInit()) {
        fprintf(stderr, "Failed to initialize GLFW\n");
        return 1;
    }
    
    printf("GLFW initialized successfully\n");
    printf("GLFW version: %s\n", glfwGetVersionString());
    
    // Request OpenGL ES context (what Minecraft uses)
    glfwWindowHint(GLFW_CLIENT_API, GLFW_OPENGL_ES_API);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 0);
    glfwWindowHint(GLFW_CONTEXT_CREATION_API, GLFW_EGL_CONTEXT_API);
    
    printf("Creating window with GLES 3.0 + EGL context...\n");
    
    GLFWwindow* window = glfwCreateWindow(640, 480, "GLFW EGL Test", NULL, NULL);
    if (!window) {
        fprintf(stderr, "Failed to create GLFW window\n");
        glfwTerminate();
        return 1;
    }
    
    printf("Window created successfully!\n");
    
    glfwMakeContextCurrent(window);
    
    printf("Context made current\n");
    
    // Quick render test
    for (int i = 0; i < 60; i++) {
        glfwSwapBuffers(window);
        glfwPollEvents();
        if (glfwWindowShouldClose(window)) break;
    }
    
    printf("Render test passed!\n");
    
    glfwDestroyWindow(window);
    glfwTerminate();
    
    printf("SUCCESS: GLFW + EGL + Wayland works!\n");
    return 0;
}
