/*
 * Camera Capture Test Program for MT8183 (MediaTek)
 * Uses the V4L2 Media Request API to capture frames
 *
 * The MT8183 camera ISP driver requires the Request API because
 * it uses an SCP (System Control Processor) for frame processing.
 * Simple v4l2 streaming won't work - frames must be queued with requests.
 *
 * Compile: gcc -o capture_camera capture_camera.c -Wall
 * Usage:   ./capture_camera <video_device> <media_device> [output.raw]
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <fcntl.h>
#include <unistd.h>
#include <errno.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <linux/videodev2.h>
#include <linux/media.h>

#define NUM_BUFFERS 4

struct buffer {
    void *start;
    size_t length;
    int fd;  /* dmabuf fd if using DMA-BUF */
};

static struct buffer buffers[NUM_BUFFERS];
static int video_fd = -1;
static int media_fd = -1;

static void print_usage(const char *prog)
{
    fprintf(stderr, "Usage: %s <video_device> <media_device> [output_file]\n", prog);
    fprintf(stderr, "  video_device: V4L2 capture device (e.g., /dev/video5)\n");
    fprintf(stderr, "  media_device: Media controller device (e.g., /dev/media0)\n");
    fprintf(stderr, "  output_file:  Output file (default: /tmp/capture.raw)\n");
    fprintf(stderr, "\nExample: %s /dev/video5 /dev/media0 photo.raw\n", prog);
}

static int xioctl(int fd, unsigned long request, void *arg)
{
    int r;
    do {
        r = ioctl(fd, request, arg);
    } while (r == -1 && errno == EINTR);
    return r;
}

static int open_devices(const char *video_dev, const char *media_dev)
{
    video_fd = open(video_dev, O_RDWR);
    if (video_fd < 0) {
        perror("Failed to open video device");
        return -1;
    }
    
    media_fd = open(media_dev, O_RDWR);
    if (media_fd < 0) {
        perror("Failed to open media device");
        close(video_fd);
        return -1;
    }
    
    printf("Opened %s (fd=%d) and %s (fd=%d)\n", 
           video_dev, video_fd, media_dev, media_fd);
    return 0;
}

static void close_devices(void)
{
    if (video_fd >= 0) close(video_fd);
    if (media_fd >= 0) close(media_fd);
}

static int query_caps(void)
{
    struct v4l2_capability cap;
    
    if (xioctl(video_fd, VIDIOC_QUERYCAP, &cap) < 0) {
        perror("VIDIOC_QUERYCAP");
        return -1;
    }
    
    printf("Driver: %s\n", cap.driver);
    printf("Card: %s\n", cap.card);
    printf("Capabilities: 0x%08x\n", cap.capabilities);
    
    if (!(cap.capabilities & V4L2_CAP_VIDEO_CAPTURE_MPLANE)) {
        fprintf(stderr, "Device doesn't support multiplanar capture\n");
        return -1;
    }
    
    if (!(cap.capabilities & V4L2_CAP_STREAMING)) {
        fprintf(stderr, "Device doesn't support streaming\n");
        return -1;
    }
    
    return 0;
}

static int set_format(int width, int height, __u32 pixelformat)
{
    struct v4l2_format fmt = {0};
    
    fmt.type = V4L2_BUF_TYPE_VIDEO_CAPTURE_MPLANE;
    fmt.fmt.pix_mp.width = width;
    fmt.fmt.pix_mp.height = height;
    fmt.fmt.pix_mp.pixelformat = pixelformat;
    fmt.fmt.pix_mp.num_planes = 1;
    fmt.fmt.pix_mp.field = V4L2_FIELD_NONE;
    
    if (xioctl(video_fd, VIDIOC_S_FMT, &fmt) < 0) {
        perror("VIDIOC_S_FMT");
        return -1;
    }
    
    printf("Format set: %dx%d, pixfmt=0x%08x, bytesperline=%d, sizeimage=%d\n",
           fmt.fmt.pix_mp.width, fmt.fmt.pix_mp.height,
           fmt.fmt.pix_mp.pixelformat,
           fmt.fmt.pix_mp.plane_fmt[0].bytesperline,
           fmt.fmt.pix_mp.plane_fmt[0].sizeimage);
    
    return fmt.fmt.pix_mp.plane_fmt[0].sizeimage;
}

static int alloc_buffers(int count)
{
    struct v4l2_requestbuffers req = {0};
    
    req.count = count;
    req.type = V4L2_BUF_TYPE_VIDEO_CAPTURE_MPLANE;
    req.memory = V4L2_MEMORY_MMAP;
    
    if (xioctl(video_fd, VIDIOC_REQBUFS, &req) < 0) {
        perror("VIDIOC_REQBUFS");
        return -1;
    }
    
    if (req.count < 2) {
        fprintf(stderr, "Insufficient buffer memory\n");
        return -1;
    }
    
    printf("Allocated %d buffers\n", req.count);
    
    for (int i = 0; i < req.count; i++) {
        struct v4l2_buffer buf = {0};
        struct v4l2_plane planes[1] = {0};
        
        buf.type = V4L2_BUF_TYPE_VIDEO_CAPTURE_MPLANE;
        buf.memory = V4L2_MEMORY_MMAP;
        buf.index = i;
        buf.m.planes = planes;
        buf.length = 1;
        
        if (xioctl(video_fd, VIDIOC_QUERYBUF, &buf) < 0) {
            perror("VIDIOC_QUERYBUF");
            return -1;
        }
        
        buffers[i].length = planes[0].length;
        buffers[i].start = mmap(NULL, planes[0].length,
                                PROT_READ | PROT_WRITE,
                                MAP_SHARED, video_fd,
                                planes[0].m.mem_offset);
        
        if (buffers[i].start == MAP_FAILED) {
            perror("mmap");
            return -1;
        }
        
        printf("Buffer %d: length=%zu, mapped at %p\n", 
               i, buffers[i].length, buffers[i].start);
    }
    
    return req.count;
}

static void free_buffers(int count)
{
    for (int i = 0; i < count; i++) {
        if (buffers[i].start && buffers[i].start != MAP_FAILED) {
            munmap(buffers[i].start, buffers[i].length);
        }
    }
}

static int queue_buffer(int index)
{
    struct v4l2_buffer buf = {0};
    struct v4l2_plane planes[1] = {0};
    
    buf.type = V4L2_BUF_TYPE_VIDEO_CAPTURE_MPLANE;
    buf.memory = V4L2_MEMORY_MMAP;
    buf.index = index;
    buf.m.planes = planes;
    buf.length = 1;
    
    if (xioctl(video_fd, VIDIOC_QBUF, &buf) < 0) {
        perror("VIDIOC_QBUF");
        return -1;
    }
    
    return 0;
}

static int start_streaming(void)
{
    enum v4l2_buf_type type = V4L2_BUF_TYPE_VIDEO_CAPTURE_MPLANE;
    
    if (xioctl(video_fd, VIDIOC_STREAMON, &type) < 0) {
        perror("VIDIOC_STREAMON");
        return -1;
    }
    
    printf("Streaming started\n");
    return 0;
}

static int stop_streaming(void)
{
    enum v4l2_buf_type type = V4L2_BUF_TYPE_VIDEO_CAPTURE_MPLANE;
    
    xioctl(video_fd, VIDIOC_STREAMOFF, &type);
    printf("Streaming stopped\n");
    return 0;
}

static int wait_for_frame(int timeout_ms)
{
    fd_set fds;
    struct timeval tv;
    int r;
    
    FD_ZERO(&fds);
    FD_SET(video_fd, &fds);
    
    tv.tv_sec = timeout_ms / 1000;
    tv.tv_usec = (timeout_ms % 1000) * 1000;
    
    r = select(video_fd + 1, &fds, NULL, NULL, &tv);
    if (r == -1) {
        if (errno == EINTR) return 0;
        perror("select");
        return -1;
    }
    if (r == 0) {
        fprintf(stderr, "Timeout waiting for frame\n");
        return -ETIMEDOUT;
    }
    
    return 1;
}

static int capture_frame(void **data, size_t *length)
{
    struct v4l2_buffer buf = {0};
    struct v4l2_plane planes[1] = {0};
    
    buf.type = V4L2_BUF_TYPE_VIDEO_CAPTURE_MPLANE;
    buf.memory = V4L2_MEMORY_MMAP;
    buf.m.planes = planes;
    buf.length = 1;
    
    if (xioctl(video_fd, VIDIOC_DQBUF, &buf) < 0) {
        perror("VIDIOC_DQBUF");
        return -1;
    }
    
    printf("Captured frame: index=%d, bytesused=%d, sequence=%d\n",
           buf.index, planes[0].bytesused, buf.sequence);
    
    *data = buffers[buf.index].start;
    *length = planes[0].bytesused;
    
    return buf.index;
}

static int save_frame(const char *filename, void *data, size_t length)
{
    FILE *fp = fopen(filename, "wb");
    if (!fp) {
        perror("fopen");
        return -1;
    }
    
    size_t written = fwrite(data, 1, length, fp);
    fclose(fp);
    
    if (written != length) {
        fprintf(stderr, "Failed to write complete frame\n");
        return -1;
    }
    
    printf("Saved %zu bytes to %s\n", written, filename);
    return 0;
}

/* 
 * Alternative: Try using Request API
 * This is needed for MT8183 ISP which requires requests
 */
static int try_request_api_capture(const char *output_file)
{
    int request_fd = -1;
    int ret = -1;
    
    printf("\n=== Trying Media Request API ===\n");
    
    /* Create a request */
    if (xioctl(media_fd, MEDIA_IOC_REQUEST_ALLOC, &request_fd) < 0) {
        perror("MEDIA_IOC_REQUEST_ALLOC");
        printf("Request API not supported - trying simple capture\n");
        return -ENOTSUP;
    }
    
    printf("Created request fd=%d\n", request_fd);
    
    /* Queue buffer with request */
    struct v4l2_buffer buf = {0};
    struct v4l2_plane planes[1] = {0};
    
    buf.type = V4L2_BUF_TYPE_VIDEO_CAPTURE_MPLANE;
    buf.memory = V4L2_MEMORY_MMAP;
    buf.index = 0;
    buf.m.planes = planes;
    buf.length = 1;
    buf.flags = V4L2_BUF_FLAG_REQUEST_FD;
    buf.request_fd = request_fd;
    
    if (xioctl(video_fd, VIDIOC_QBUF, &buf) < 0) {
        perror("VIDIOC_QBUF with request");
        goto cleanup;
    }
    
    printf("Buffer queued with request\n");
    
    /* Queue the request */
    if (xioctl(request_fd, MEDIA_REQUEST_IOC_QUEUE, NULL) < 0) {
        perror("MEDIA_REQUEST_IOC_QUEUE");
        goto cleanup;
    }
    
    printf("Request queued, waiting for completion...\n");
    
    /* Start streaming if not already */
    if (start_streaming() < 0) {
        goto cleanup;
    }
    
    /* Wait for completion using poll on request_fd */
    fd_set fds;
    struct timeval tv = { .tv_sec = 5, .tv_usec = 0 };
    
    FD_ZERO(&fds);
    FD_SET(request_fd, &fds);
    
    int r = select(request_fd + 1, NULL, NULL, &fds, &tv);
    if (r <= 0) {
        fprintf(stderr, "Timeout or error waiting for request: %d\n", r);
        goto cleanup_stream;
    }
    
    printf("Request completed!\n");
    
    /* Dequeue the buffer */
    void *data;
    size_t length;
    int idx = capture_frame(&data, &length);
    if (idx >= 0) {
        save_frame(output_file, data, length);
        queue_buffer(idx);  /* Re-queue buffer */
        ret = 0;
    }

cleanup_stream:
    stop_streaming();
    
cleanup:
    if (request_fd >= 0) {
        /* Reinit for reuse */
        xioctl(request_fd, MEDIA_REQUEST_IOC_REINIT, NULL);
        close(request_fd);
    }
    
    return ret;
}

int main(int argc, char *argv[])
{
    const char *video_dev;
    const char *media_dev;
    const char *output_file = "/tmp/capture.raw";
    int ret = 1;
    int buf_count;
    
    if (argc < 3 || strcmp(argv[1], "-h") == 0 || strcmp(argv[1], "--help") == 0) {
        print_usage(argv[0]);
        return (argc < 3) ? 1 : 0;
    }
    
    video_dev = argv[1];
    media_dev = argv[2];
    if (argc > 3) {
        output_file = argv[3];
    }
    
    printf("=== MT8183 Camera Capture Test ===\n");
    printf("Video device: %s\n", video_dev);
    printf("Media device: %s\n", media_dev);
    printf("Output file: %s\n", output_file);
    printf("===================================\n\n");
    
    if (open_devices(video_dev, media_dev) < 0)
        goto exit;
    
    if (query_caps() < 0)
        goto cleanup;
    
    /* Set format: 8-bit GRBG packed (MBg8 for MT8183) */
    /* v4l2_fourcc('M', 'B', 'g', '8') = 0x3867424d */
    int sizeimage = set_format(3280, 2464, 0x3867424d);
    if (sizeimage < 0)
        goto cleanup;
    
    buf_count = alloc_buffers(NUM_BUFFERS);
    if (buf_count < 0)
        goto cleanup;
    
    /* Try Request API first (required for MT8183) */
    ret = try_request_api_capture(output_file);
    if (ret == 0) {
        printf("\nCapture successful using Request API!\n");
        goto cleanup_buffers;
    }
    
    if (ret != -ENOTSUP) {
        goto cleanup_buffers;
    }
    
    /* Fallback: Simple capture (may not work on MT8183) */
    printf("\n=== Trying Simple Capture ===\n");
    printf("Note: This may not work with MT8183 ISP\n");
    
    /* Queue all buffers */
    for (int i = 0; i < buf_count; i++) {
        if (queue_buffer(i) < 0)
            goto cleanup_buffers;
    }
    
    if (start_streaming() < 0)
        goto cleanup_buffers;
    
    /* Wait for frame with 5 second timeout */
    int wait_ret = wait_for_frame(5000);
    if (wait_ret <= 0) {
        fprintf(stderr, "No frame received (ISP may require Request API)\n");
        goto cleanup_stream;
    }
    
    void *data;
    size_t length;
    int idx = capture_frame(&data, &length);
    if (idx >= 0) {
        save_frame(output_file, data, length);
        ret = 0;
    }
    
cleanup_stream:
    stop_streaming();
    
cleanup_buffers:
    free_buffers(buf_count);
    
cleanup:
    close_devices();
    
exit:
    return ret;
}
