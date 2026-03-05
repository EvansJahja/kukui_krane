/*
 * test_glfw_gl32.c - Test Desktop OpenGL 3.2 Core (Minecraft minimum)
 * 
 * Minecraft 1.17+ requires OpenGL 3.2 Core minimum
 */

#include <GLFW/glfw3.h>
#include <stdio.h>
#include <stdlib.h>

void error_callback(int error, const char* description) {
    fprintf(stderr, "GLFW error %d: %s\n", error, description);
}

int main(void) {
    glfwSetErrorCallback(error_callback);
    
    printf("Testing GLFW with OpenGL 3.2 Core (Minecraft minimum)...\n");
    
    if (!glfwInit()) {
        fprintf(stderr, "Failed to initialize GLFW\n");
        return 1;
    }
    
    printf("GLFW version: %s\n", glfwGetVersionString());
    
    // Request OpenGL 3.2 Core (Minecraft's minimum requirement)
    glfwWindowHint(GLFW_CLIENT_API, GLFW_OPENGL_API);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 2);
    glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);
    glfwWindowHint(GLFW_CONTEXT_CREATION_API, GLFW_EGL_CONTEXT_API);
    
    printf("Creating window with OpenGL 3.2 Core + EGL...\n");
    
    GLFWwindow* window = glfwCreateWindow(640, 480, "GL 3.2 Core Test", NULL, NULL);
    if (!window) {
        fprintf(stderr, "Failed to create OpenGL 3.2 Core window\n");
        
        // Try without Core profile
        printf("\nTrying OpenGL 3.2 without Core profile...\n");
        glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_ANY_PROFILE);
        window = glfwCreateWindow(640, 480, "GL 3.2 Test", NULL, NULL);
        if (!window) {
            fprintf(stderr, "Failed to create OpenGL 3.2 window\n");
            glfwTerminate();
            return 1;
        }
    }
    
    printf("SUCCESS: OpenGL 3.2 context works!\n");
    glfwDestroyWindow(window);
    glfwTerminate();
    return 0;
}
