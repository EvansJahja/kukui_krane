/*
 * test_glfw_gl21.c - Test Desktop OpenGL 2.1 (compatibility profile)
 * 
 * Minecraft can fall back to GL 2.1 for software rendering
 */

#include <GLFW/glfw3.h>
#include <stdio.h>
#include <stdlib.h>

void error_callback(int error, const char* description) {
    fprintf(stderr, "GLFW error %d: %s\n", error, description);
}

int main(void) {
    glfwSetErrorCallback(error_callback);
    
    printf("Testing GLFW with OpenGL 2.1...\n");
    
    if (!glfwInit()) {
        fprintf(stderr, "Failed to initialize GLFW\n");
        return 1;
    }
    
    printf("GLFW version: %s\n", glfwGetVersionString());
    
    // Request OpenGL 2.1 (no profile specification)
    glfwWindowHint(GLFW_CLIENT_API, GLFW_OPENGL_API);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 2);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 1);
    glfwWindowHint(GLFW_CONTEXT_CREATION_API, GLFW_EGL_CONTEXT_API);
    
    printf("Creating window with OpenGL 2.1 + EGL...\n");
    
    GLFWwindow* window = glfwCreateWindow(640, 480, "GL 2.1 Test", NULL, NULL);
    if (!window) {
        fprintf(stderr, "Failed to create OpenGL 2.1 window\n");
        glfwTerminate();
        return 1;
    }
    
    printf("SUCCESS: OpenGL 2.1 context works!\n");
    glfwDestroyWindow(window);
    glfwTerminate();
    return 0;
}
