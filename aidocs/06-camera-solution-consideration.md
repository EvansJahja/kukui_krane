# Camera Solution Consideration for MT8183 Kukui Krane

## Background

### Hardware Overview

The MT8183 Kukui Krane Chromebook has two cameras:

| Camera | Sensor | Resolution | Interface | Features |
|--------|--------|------------|-----------|----------|
| Rear | OV8856 | 3280×2464 (8MP) | MIPI CSI-2 | Autofocus capable |
| Front | OV02A10 | 1600×1200 (2MP) | MIPI CSI-2 | Fixed focus |

### Sensor Data Format

Both sensors output raw Bayer data:
- **OV8856 (rear)**: SGRBG10 (10-bit Bayer, GRBG pattern)
- **OV02A10 (front)**: SGRBG10 (10-bit Bayer, GRBG pattern)

The ISP (`mtk-cam-p1`) can output in multiple formats:
- Raw 10-bit packed
- Raw 8-bit (GRBG8) - what we currently use
- Various metadata formats

### Media Pipeline Architecture

```
Sensor (ov8856/ov02a10)
         ↓
    SENINF Bridge (1a040000.seninf)
         ↓
    ISP (mtk-cam-p1)
         ↓
    Video Output Nodes
    ├── /dev/video3 (main stream - capture)
    ├── /dev/video2 (meta input - tuning params)
    ├── /dev/video5 (packed out)
    └── /dev/video6-9 (partial metadata)
```

### Current Achievement

We have successfully captured images from the rear camera using a Python script (`capture.py`) that:

1. Auto-detects media devices and video nodes
2. Configures the media pipeline (sensor → SENINF → ISP)
3. Uses the **Media Request API** for synchronized capture
4. Outputs raw Bayer data as PPM images

Sample capture workflow:
```bash
./capture.py                    # Captures to output_YYYYMMDD_HHMMSS.ppm
./capture.py --output test.ppm  # Captures to specified file
```

---

## Technical Considerations

### Why Standard V4L2 Doesn't Work

Traditional V4L2 webcam access follows a simple pattern:
```
open(/dev/video0) → VIDIOC_STREAMON → read frames → close
```

This works for **UVC webcams** that provide processed, ready-to-use frames. However, the MT8183 camera subsystem uses the **Media Request API** which requires:

1. Opening a media device (`/dev/media0`)
2. Configuring subdevices via media-ctl
3. Allocating request objects
4. Queuing buffers **with** request FDs
5. Submitting requests and waiting for completion

Applications like Firefox, GNOME Snapshot, and Cheese expect simple V4L2 devices and **cannot** use Media Request API cameras.

### How Google's Chrome OS Camera Stack Works

Chrome OS has a sophisticated camera architecture:

```
┌─────────────────────────────────────────────────────────┐
│                    Chrome Browser                        │
│                         ↓                                │
│              Camera HAL Adapter (Mojo IPC)               │
│                         ↓                                │
│           MediaTek Camera HAL (cros-camera-hal-mtk)      │
│    ┌────────────────────┼────────────────────┐          │
│    │                    │                    │          │
│    │  3A Algorithms     │   mtklibv4l2      │          │
│    │  (proprietary)     │   (V4L2 wrapper)  │          │
│    └────────────────────┴────────────────────┘          │
│                         ↓                                │
│              Kernel: mtk-cam-p1 driver                   │
└─────────────────────────────────────────────────────────┘
```

**Why this doesn't work outside Chrome OS:**
- The Camera HAL is tightly integrated with Chrome's Mojo IPC
- 3A libraries are proprietary and sandboxed
- The HAL expects Chrome OS-specific services
- No standard Linux camera interface is exposed

### libcamera Status

libcamera is the modern Linux camera framework that could theoretically support MT8183, but:
- **No MT8183 pipeline handler exists** (as of March 2026)
- Writing one requires significant reverse-engineering
- The IPU3 and RkISP1 handlers took years to develop
- Community resources for MediaTek ISPs are limited

### Megapixels/libmegapixels Attempt

We attempted to use Megapixels (from postmarketOS) which supports Media Request API cameras. However, libmegapixels has a critical limitation:

**The Problem:**
```c
// In parse.c - find_media_node()
for (int i = 0; i < topology.num_entities; i++) {
    if (entities[i].function == MEDIA_ENT_F_IO_V4L) {
        // Takes FIRST entity with this function
        camera->video_path = find_path_for_devnode(...);
        break;  // ← Always picks first match!
    }
}
```

On MT8183, the first `MEDIA_ENT_F_IO_V4L` entity is:
- **Entity 14**: `mtk-cam-p1 meta input` → `/dev/video2` (metadata, NOT capture!)

But we need:
- **Entity 20**: `mtk-cam-p1 main stream` → `/dev/video3` (actual video capture)

libmegapixels doesn't provide a way to specify which video entity to use when an ISP has multiple video nodes.

### Why v4l2loopback is the Pragmatic Solution

Given the constraints:
- ✗ Standard V4L2 doesn't support Media Request API
- ✗ Chrome OS HAL isn't portable
- ✗ libcamera has no MT8183 support
- ✗ libmegapixels picks the wrong video node

**v4l2loopback** allows us to:
- Create a virtual `/dev/video` device
- Feed it frames from our working `capture.py`
- Present a "normal" webcam to all applications
- Firefox, Cheese, Zoom, etc. all work transparently

---

## Implementation Milestones

### Phase 1: Kernel Module Fixes ✓ (Completed)

**Problem:** Camera modules weren't loading automatically.

**Solution:** Created module loading script:
```bash
# Current workaround - manual module loading
modprobe videobuf2_memops
modprobe videobuf2_v4l2
modprobe videobuf2_dma_contig
modprobe v4l2_fwnode
modprobe ov8856
modprobe ov02a10
modprobe mtk_cam_isp
```

**TODO:** Proper udev rules or systemd service for automatic loading.

### Phase 2: capture.py Development ✓ (Completed)

Created a Python script that:
- Auto-discovers camera hardware via `/dev/media*`
- Sets up media pipeline links
- Configures sensor resolution and format
- Uses Media Request API for capture
- Outputs PPM images

**Current capabilities:**
- Rear camera (OV8856) capture working
- 3280×2464 resolution at ~15fps potential
- Raw Bayer to RGB conversion (basic)

### Phase 3: v4l2loopback Service (Next Step)

**Goal:** Create a background service that:
1. Loads `v4l2loopback` kernel module
2. Creates virtual camera device (`/dev/video10` or similar)
3. Runs capture loop, feeding frames to virtual device
4. Activates on-demand when applications open the camera

**Architecture:**
```
┌─────────────────────────────────────────────────────┐
│                    User Application                  │
│              (Firefox, Cheese, Zoom, etc.)          │
│                         ↓                            │
│              /dev/video10 (v4l2loopback)            │
│                         ↑                            │
│              camera-service (Python daemon)          │
│                         ↓                            │
│              capture.py core logic                   │
│                         ↓                            │
│              /dev/media0 + /dev/video3              │
│                  (Media Request API)                 │
└─────────────────────────────────────────────────────┘
```

**Implementation tasks:**
1. Install v4l2loopback-dkms
2. Create systemd service unit
3. Implement frame loop with proper timing
4. Handle device open/close events
5. Convert Bayer → YUV420/RGB for loopback

### Phase 4: Front Camera Support (Future)

**Current state:** Front camera (OV02A10) has never been tested.

**Questions to answer:**
- Does it appear as a separate media device or share `/dev/media0`?
- What is its video node path?
- Does the same pipeline setup work?

**Investigation needed:**
```bash
# Check if front camera is enumerated
media-ctl -d /dev/media0 -p | grep -i ov02a10

# Or might be separate media device
ls /dev/media*
media-ctl -d /dev/media1 -p  # if exists
```

**Approach:**
1. First, get front camera detection working in capture.py
2. Add camera selection (rear/front) to the service
3. Consider whether to expose as single or dual virtual cameras

### Phase 5: 3A Integration (Final Stage)

**Autofocus (AF):**
- OV8856 has VCM (Voice Coil Motor) for autofocus
- Requires V4L2 focus controls
- May need contrast-detection algorithm

**Auto Exposure (AE):**
- Sensor has exposure time and gain controls
- Need histogram analysis of captured frames
- Adjust exposure to target brightness

**Auto White Balance (AWB):**
- Analyze color channels in captured image
- Apply color correction matrix
- May use gray-world or more sophisticated algorithms

**Implementation approach:**
1. First get stable capture working (Phases 1-4)
2. Add basic manual controls
3. Implement simple AE (exposure adjustment based on brightness)
4. Implement simple AWB (gray-world algorithm)
5. Consider AF last (most complex, requires focus sweep)

---

## Summary: Incremental Development Strategy

We've deliberately taken a careful, step-by-step approach:

| Step | Status | Description |
|------|--------|-------------|
| 1 | ✓ | Understand hardware (sensors, ISP, media topology) |
| 2 | ✓ | Get kernel modules loading |
| 3 | ✓ | Create working capture script |
| 4 | → | v4l2loopback virtual camera (rear only) |
| 5 | ○ | Front camera investigation |
| 6 | ○ | Dual camera support |
| 7 | ○ | Basic 3A (AE, AWB) |
| 8 | ○ | Autofocus support |

Each milestone builds on the previous, ensuring we have a working system at each stage rather than attempting everything at once and debugging a complex broken system.

---

## References

- [Media Request API documentation](https://www.kernel.org/doc/html/latest/userspace-api/media/mediactl/request-api.html)
- [v4l2loopback GitHub](https://github.com/umlaeute/v4l2loopback)
- [Chrome OS Camera HAL](https://chromium.googlesource.com/chromiumos/platform2/+/HEAD/camera/README.md)
- [libcamera](https://libcamera.org/)
- [Megapixels](https://gitlab.com/megapixels-org/Megapixels)
