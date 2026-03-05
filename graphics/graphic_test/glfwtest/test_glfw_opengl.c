/*
 * test_glfw_opengl.c - GLFW test with desktop OpenGL (not ES)
 * 
 * Compile: gcc test_glfw_opengl.c -o test_glfw_opengl -lglfw
 * Run:     EGL_LOG_LEVEL=debug ./test_glfw_opengl
 */

#include <GLFW/glfw3.h>
#include <stdio.h>
#include <stdlib.h>

void error_callback(int error, const char* description) {
    fprintf(stderr, "GLFW error %d: %s\n", error, description);
}

int main(void) {
    glfwSetErrorCallback(error_callback);
    
    printf("Testing GLFW with desktop OpenGL...\n");
    printf("WAYLAND_DISPLAY=%s\n", getenv("WAYLAND_DISPLAY") ?: "(not set)");
    
    if (!glfwInit()) {
        fprintf(stderr, "Failed to initialize GLFW\n");
        return 1;
    }
    
    printf("GLFW initialized successfully\n");
    printf("GLFW version: %s\n", glfwGetVersionString());
    
    // Use desktop OpenGL instead of ES
    glfwWindowHint(GLFW_CLIENT_API, GLFW_OPENGL_API);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 3);
    glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);
    glfwWindowHint(GLFW_CONTEXT_CREATION_API, GLFW_EGL_CONTEXT_API);
    
    printf("Creating window with OpenGL 3.3 Core + EGL...\n");
    
    GLFWwindow* window = glfwCreateWindow(640, 480, "GLFW OpenGL Test", NULL, NULL);
    if (!window) {
        fprintf(stderr, "Failed to create GLFW window\n");
        glfwTerminate();
        return 1;
    }
    
    printf("Window created successfully!\n");
    
    glfwMakeContextCurrent(window);
    printf("SUCCESS: Desktop OpenGL context works!\n");
    
    glfwDestroyWindow(window);
    glfwTerminate();
    return 0;
}
