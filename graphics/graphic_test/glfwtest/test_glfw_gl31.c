/*
 * test_glfw_gl31.c - Test Desktop OpenGL 3.1 (Panfrost maximum)
 */

#include <GLFW/glfw3.h>
#include <stdio.h>
#include <stdlib.h>

void error_callback(int error, const char* description) {
    fprintf(stderr, "GLFW error %d: %s\n", error, description);
}

int main(void) {
    glfwSetErrorCallback(error_callback);
    
    printf("Testing GLFW with OpenGL 3.1 (Panfrost max)...\n");
    
    if (!glfwInit()) {
        fprintf(stderr, "Failed to initialize GLFW\n");
        return 1;
    }
    
    printf("GLFW version: %s\n", glfwGetVersionString());
    
    // OpenGL 3.1 - max supported by panfrost on MT8183
    glfwWindowHint(GLFW_CLIENT_API, GLFW_OPENGL_API);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 1);
    glfwWindowHint(GLFW_CONTEXT_CREATION_API, GLFW_EGL_CONTEXT_API);
    
    printf("Creating window with OpenGL 3.1 + EGL...\n");
    
    GLFWwindow* window = glfwCreateWindow(640, 480, "GL 3.1 Test", NULL, NULL);
    if (!window) {
        fprintf(stderr, "Failed to create OpenGL 3.1 window\n");
        glfwTerminate();
        return 1;
    }
    
    printf("SUCCESS: OpenGL 3.1 context works!\n");
    glfwDestroyWindow(window);
    glfwTerminate();
    return 0;
}
