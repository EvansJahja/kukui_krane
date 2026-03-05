/*
 * test_glfw_native.c - GLFW test with native context API (let GLFW choose)
 * 
 * Compile: gcc test_glfw_native.c -o test_glfw_native -lglfw
 * Run:     EGL_LOG_LEVEL=debug ./test_glfw_native
 */

#include <GLFW/glfw3.h>
#include <stdio.h>
#include <stdlib.h>

void error_callback(int error, const char* description) {
    fprintf(stderr, "GLFW error %d: %s\n", error, description);
}

int main(void) {
    glfwSetErrorCallback(error_callback);
    
    printf("Testing GLFW with native context API...\n");
    printf("WAYLAND_DISPLAY=%s\n", getenv("WAYLAND_DISPLAY") ?: "(not set)");
    
    if (!glfwInit()) {
        fprintf(stderr, "Failed to initialize GLFW\n");
        return 1;
    }
    
    printf("GLFW initialized successfully\n");
    printf("GLFW version: %s\n", glfwGetVersionString());
    
    // Let GLFW choose the context API (native = auto-detect)
    glfwWindowHint(GLFW_CLIENT_API, GLFW_OPENGL_API);
    glfwWindowHint(GLFW_CONTEXT_CREATION_API, GLFW_NATIVE_CONTEXT_API);
    
    printf("Creating window with native context API...\n");
    
    GLFWwindow* window = glfwCreateWindow(640, 480, "GLFW Native Test", NULL, NULL);
    if (!window) {
        fprintf(stderr, "Failed to create GLFW window\n");
        glfwTerminate();
        return 1;
    }
    
    printf("Window created successfully!\n");
    
    glfwMakeContextCurrent(window);
    printf("SUCCESS: Native context API works!\n");
    
    glfwDestroyWindow(window);
    glfwTerminate();
    return 0;
}
