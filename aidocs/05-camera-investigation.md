# Making the MT8183 Camera Work: A Technical Investigation

**Platform:** MediaTek MT8183 Kukui Krane (ChromeBook)  
**Date:** 2026-03-05  
**Final Status:** ✅ Working

---

## Executive Summary

The cameras on this ChromeBook weren't working. After investigation, we discovered:

1. **A driver bug** - An uninitialized variable caused random failures
2. **The wrong capture method** - Standard V4L2 streaming tools don't work; this ISP requires the **Media Request API**

**Quick test (after reboot):**
```bash
cd /home/evans/linux-root/linux/chromeos-6.12/aidocs

# Load the patched camera module (run once after each reboot)
sudo ./load_module.sh

# Capture photo (auto-detects devices, outputs PPM)
./capture.py

# View result
xdg-open /tmp/preview.ppm  # or copy to another machine

# For PNG output (requires ImageMagick):
./capture.py --png
```

---

# Part 1: Understanding the Camera System

## 1.1 What is V4L2?

**V4L2** (Video4Linux2) is the Linux kernel's framework for video capture devices. When you plug in a webcam or have a built-in camera, V4L2 provides a standardized way for applications to:

- Discover cameras (`/dev/video0`, `/dev/video1`, etc.)
- Configure resolution and format
- Capture frames

Think of V4L2 as a "language" that applications speak to talk to cameras.

## 1.2 Simple vs Complex Camera Systems

### Simple USB Webcam
```
┌──────────┐      ┌───────────────┐
│  Sensor  │ ───► │ /dev/video0   │ ───► Application
└──────────┘      └───────────────┘
```
A USB webcam is self-contained. You open `/dev/video0`, request frames, and get images.

### Complex SoC Camera (like MT8183)
```
┌──────────┐      ┌──────────┐      ┌─────────┐      ┌─────────────┐
│  Sensor  │ ───► │  SENINF  │ ───► │   ISP   │ ───► │ /dev/video2 │
│ (ov8856) │ MIPI │ (bridge) │      │(mtk-cam)│      │             │
└──────────┘      └──────────┘      └─────────┘      └─────────────┘
     │                                    │
     │                               ┌────┴────┐
     │                               │   SCP   │
     │                               │(firmware│
     │                               │processor│
     └───────────────────────────────┴─────────┘
```

Mobile SoCs have sophisticated **Image Signal Processors (ISPs)**. The MT8183 ISP:
- Connects to the sensor via **MIPI CSI-2** (a high-speed serial bus)
- Uses a separate processor called **SCP** (System Control Processor) to handle frame processing
- Exposes multiple `/dev/video*` devices for different output types

## 1.3 What is the Media Controller?

Because complex cameras have multiple components, Linux provides the **Media Controller** framework. It lets you:

- See the topology: `media-ctl -d /dev/media0 -p`
- Connect components: `media-ctl -l` (create links)
- Configure formats: `media-ctl -V` (set video format)

Think of it as a **patchbay** where you wire up components before capturing.

## 1.4 Our Hardware

| Component | Description | Device |
|-----------|-------------|--------|
| **OV8856** | Back camera sensor, 8MP | I2C address 2-0010 |
| **OV02A10** | Front camera sensor, 2MP | I2C address 4-003d |
| **SENINF** | Sensor interface (MIPI receiver) | 1a040000.seninf |
| **ISP** | Image Signal Processor | mtk-cam-p1 |
| **SCP** | Coprocessor running ISP firmware | 10500000.scp |

---

# Part 2: The Problem

## 2.1 Symptom

Running the standard capture command:
```bash
v4l2-ctl -d /dev/video2 --stream-mmap --stream-count=1 --stream-to=photo.raw
```

**Result:** Hangs forever. No image captured. Sometimes causes kernel crash.

## 2.2 Initial Errors Observed

```
failed to start pipeline:-32   # EPIPE - "broken pipe"
VIDIOC_STREAMON: Device or resource busy
```

---

# Part 3: Debugging Journey (Walkthrough)

Follow these steps to rediscover the issues yourself.

**Note:** Device paths like `/dev/video3` and `/dev/media0` vary between boots! The examples below use placeholder paths - use `v4l2-ctl --list-devices` to find your actual device paths, or just use `capture.py` which auto-detects everything.

## Step 1: Verify Hardware Detection

First, check that the kernel sees the cameras:

```bash
# List all video devices
v4l2-ctl --list-devices
```

Expected output (device numbers vary each boot!):
```
mtk-cam-p1 (platform:1a000000.camisp):
        /dev/video2      ◄── meta input
        /dev/video3      ◄── main stream (use this!)
        /dev/video5      ◄── packed out
        ...
        /dev/media0      ◄── media controller
```

```bash
# Check I2C devices (sensors)
ls /sys/bus/i2c/devices/
```

Look for `2-0010` (ov8856) and `4-003d` (ov02a10).

**If these don't appear:** Hardware or device tree issue. Stop here.

## Step 2: Examine the Media Pipeline

Find your media device first:
```bash
# Find which /dev/mediaX belongs to mtk-cam-p1
v4l2-ctl --list-devices | grep -A20 "mtk-cam-p1" | grep media
```

Then examine the topology:
```bash
media-ctl -d /dev/media0 -p   # Use YOUR media device!
```

This shows all entities and their connections. Key things to look for:

```
- entity 69: ov8856 2-0010 (1 pad, 1 link)
        pad0: SOURCE
                -> "1a040000.seninf":0 []   ◄── Link exists but NOT enabled
```

The `[]` means disabled. `[ENABLED]` means active.

## Step 3: Enable the Sensor Link

The camera sensor must be connected to the SENINF (use your actual media device):

```bash
# Find your media device first!
MEDIA_DEV=$(v4l2-ctl --list-devices | grep -A20 "mtk-cam-p1" | grep "/dev/media" | tr -d '[:space:]')
echo "Using: $MEDIA_DEV"

media-ctl -d $MEDIA_DEV -l "'ov8856 2-0010':0 -> '1a040000.seninf':0 [1]"
```

Verify:
```bash
media-ctl -d /dev/media0 -p | grep ov8856 -A5
```

Should show `[ENABLED]`.

## Step 4: Configure Format Chain

Every component in the chain needs matching format. The sensor outputs **SGRBG10** (10-bit raw Bayer).

**Note:** Use the media device you found in Step 2!

```bash
# Set sensor output format
media-ctl -d $MEDIA_DEV -V "'ov8856 2-0010':0 [fmt:SGRBG10_1X10/3280x2464]"

# Set SENINF input (must match sensor)
media-ctl -d $MEDIA_DEV -V "'1a040000.seninf':0 [fmt:SGRBG10_1X10/3280x2464]"

# Set SENINF output to ISP
media-ctl -d $MEDIA_DEV -V "'1a040000.seninf':4 [fmt:SGRBG10_1X10/3280x2464]"

# Find main stream video device
VIDEO_DEV=$(media-ctl -d $MEDIA_DEV -p | grep -A1 "main stream" | grep "video" | sed 's/.*\(\/dev\/video[0-9]*\).*/\1/')
echo "Main stream: $VIDEO_DEV"

# Set video device format
v4l2-ctl -d $VIDEO_DEV --set-fmt-video=width=3280,height=2464,pixelformat='MBgA'
```

**Why 'MBgA'?** That's the FourCC code for "10-bit Bayer GRBG MTISP Packed" format.

## Step 5: Try Standard Capture (This Will Fail)

```bash
# Use your actual video device from Step 4!
v4l2-ctl -d $VIDEO_DEV --stream-mmap=4 --stream-count=1
```

**What happens:** It hangs. No frames arrive. Why?

Check interrupts:
```bash
cat /proc/interrupts | grep cam
```

The interrupt count doesn't increase during capture. The ISP isn't receiving data!

## Step 6: Check Kernel Messages

```bash
dmesg | tail -50
```

Look for:
- `seninf s_stream returned 0` ✓ (SENINF started OK)
- `sensor s_stream returned 0` ✓ (Sensor started OK)
- Any errors?

Everything looks OK but no frames. **This is the key mystery.**

---

# Part 4: Root Cause Analysis

## 4.1 Bug #1: Uninitialized Variable (Driver Bug)

In `drivers/media/platform/mediatek/isp/isp_50/cam/mtk_cam.c` around line 608:

**Before (buggy):**
```c
struct v4l2_subdev_format sd_fmt;
// ... sd_fmt.pad is UNINITIALIZED (contains garbage)
ret = v4l2_subdev_call(cam->sensor, pad, get_fmt, NULL, &sd_fmt);
```

The `pad` field contains random stack garbage. If it happens to be non-zero, the sensor driver returns `-EINVAL` because sensors typically only have pad 0.

**After (fixed):**
```c
struct v4l2_subdev_format sd_fmt = { };  // Zero-initialize everything
sd_fmt.which = V4L2_SUBDEV_FORMAT_ACTIVE;
sd_fmt.pad = 0;  // Explicitly set pad 0
ret = v4l2_subdev_call(cam->sensor, pad, get_fmt, NULL, &sd_fmt);
```

### How to Apply the Fix

```bash
cd /home/evans/linux-root/linux/chromeos-6.12

# Edit the file (the fix should already be applied)
# Look around line 603-610 in:
#   drivers/media/platform/mediatek/isp/isp_50/cam/mtk_cam.c

# Rebuild just the camera module
make -j8 M=drivers/media/platform/mediatek/isp/isp_50/cam modules

# Load the fixed module
sudo rmmod mtk_cam_isp 2>/dev/null
sudo insmod drivers/media/platform/mediatek/isp/isp_50/cam/mtk-cam-isp.ko
```

## 4.2 Bug #2: Standard V4L2 Streaming Doesn't Work

**This is the BIG one.**

Even with Bug #1 fixed, `v4l2-ctl --stream-mmap` doesn't work. The frames never arrive.

### Why?

The MT8183 ISP uses an **asynchronous coprocessor (SCP)** to process frames. The standard V4L2 streaming model is:

```
Application                            Driver
    │                                     │
    ├──► VIDIOC_REQBUFS ──────────────────┤
    ├──► VIDIOC_QBUF ─────────────────────┤
    ├──► VIDIOC_STREAMON ─────────────────┤
    │                                     │
    │         (wait for frame)            │
    │                                     ◄── Frame arrives from HW
    ◄──────── VIDIOC_DQBUF ───────────────┤
```

But the MT8183 ISP requires the **Media Request API**:

```
Application                            Driver                  SCP
    │                                     │                      │
    ├──► MEDIA_IOC_REQUEST_ALLOC ────────►│                      │
    ├──► VIDIOC_QBUF (with request_fd) ──►│                      │
    ├──► MEDIA_REQUEST_IOC_QUEUE ────────►├─── IPI message ─────►│
    ├──► VIDIOC_STREAMON ────────────────►│                      │
    │                                     │                      │
    │         (wait on request_fd)        │    ◄── SCP processes │
    │                                     ◄─── Frame done ───────┤
    ◄──────── VIDIOC_DQBUF ───────────────┤                      │
```

The **Request API** groups buffers into atomic "requests". The SCP only processes frames when requests are explicitly queued.

### Why v4l2-ctl Fails

`v4l2-ctl` uses the standard streaming model. It never creates requests or calls `MEDIA_REQUEST_IOC_QUEUE`. So:

1. Buffers get queued ✓
2. STREAMON is called ✓
3. But SCP never receives work to do ✗
4. ISP sits idle, no interrupts, no frames ✗

---

# Part 5: The Solution

## 5.1 Using the Media Request API

We need to write code that:

1. Creates a request: `ioctl(media_fd, MEDIA_IOC_REQUEST_ALLOC, &request_fd)`
2. Queues buffer with request: `buf.flags = V4L2_BUF_FLAG_REQUEST_FD; buf.request_fd = request_fd`
3. Submits the request: `ioctl(request_fd, MEDIA_REQUEST_IOC_QUEUE, NULL)`
4. Waits for completion (select/poll on request_fd)
5. Dequeues the frame: `VIDIOC_DQBUF`

## 5.2 The Test Program

A working implementation is in `capture_camera.c`. Key excerpts:

```c
// Create a media request
int request_fd;
ioctl(media_fd, MEDIA_IOC_REQUEST_ALLOC, &request_fd);

// Queue buffer WITH the request attached
struct v4l2_buffer buf = {0};
buf.type = V4L2_BUF_TYPE_VIDEO_CAPTURE_MPLANE;
buf.memory = V4L2_MEMORY_MMAP;
buf.flags = V4L2_BUF_FLAG_REQUEST_FD;   // ← Key flag
buf.request_fd = request_fd;             // ← Attach request
ioctl(video_fd, VIDIOC_QBUF, &buf);

// Submit the request to SCP
ioctl(request_fd, MEDIA_REQUEST_IOC_QUEUE, NULL);

// Start streaming
ioctl(video_fd, VIDIOC_STREAMON, &type);

// Wait for request completion
fd_set fds;
FD_SET(request_fd, &fds);
select(request_fd + 1, NULL, NULL, &fds, &timeout);

// Get the frame
ioctl(video_fd, VIDIOC_DQBUF, &buf);
// buf now points to captured frame data!
```

## 5.3 Compile and Run

```bash
cd /home/evans/linux-root/linux/chromeos-6.12/aidocs

# Compile
gcc -o capture_camera capture_camera.c -Wall

# Run (requires video device AND media device as arguments)
./capture_camera /dev/video5 /dev/media0 /tmp/photo.raw

# Check the result
ls -la /tmp/photo.raw   # Should be ~8MB for 8-bit format
hexdump -C /tmp/photo.raw | head  # Should show image data, not zeros
```

**Usage:**
```
./capture_camera <video_device> <media_device> [output_file]
  video_device: V4L2 capture device (e.g., /dev/video5)
  media_device: Media controller device (e.g., /dev/media0)
  output_file:  Output file (default: /tmp/capture.raw)
```

**Expected output:**
```
=== MT8183 Camera Capture Test ===
Video device: /dev/video5
Media device: /dev/media0
Output file: /tmp/photo.raw
===================================

Opened /dev/video5 (fd=3) and /dev/media0 (fd=4)
Driver: mtk-cam-p1
Card: mtk-cam-p1
...
=== Trying Media Request API ===
Created request fd=5
Buffer queued with request
Request queued, waiting for completion...
Streaming started
Request completed!
Captured frame: index=0, bytesused=8081920, sequence=1
Saved 8081920 bytes to /tmp/photo.raw

Capture successful using Request API!
```

---

# Part 6: Understanding the Output

## 6.1 Pixel Formats

The MT8183 ISP outputs **MTISP Packed** format - a MediaTek-proprietary packing scheme. Standard raw viewers won't understand this format directly.

### Available Formats

| FourCC | Description | Bytes/pixel | Notes |
|--------|-------------|-------------|-------|
| `MBg8` | 8-bit Bayer GRBG | 1.0 | **Recommended** - easy to process |
| `MBgA` | 10-bit Bayer GRBG | 1.25 | Higher quality, harder to decode |
| `MBgC` | 12-bit Bayer GRBG | 1.5 | Maximum quality |
| `MBgE` | 14-bit Bayer GRBG | 1.75 | Rarely needed |

### 8-bit vs 10-bit: Which to Use?

**8-bit (`MBg8`) - Recommended for most uses:**
- Simple: 1 byte per pixel, standard Bayer layout
- Easy conversion: Can be processed with basic Python or `convert`
- File size: 3280 × 2464 = ~8 MB
- Sufficient for most photography needs

**10-bit (`MBgA`) - For maximum quality:**
- 4× more tonal gradations (1024 vs 256 levels)
- Complex packing: 4 pixels → 5 bytes (MTISP-specific)
- File size: ~10 MB
- Requires custom decoder or specialized software
- Use when: HDR processing, professional editing, or low-light recovery

The sensor natively captures 10-bit, but the ISP can convert to 8-bit output. We use 8-bit for convenience since MTISP 10-bit packing is non-standard.

## 6.2 Raw Bayer Pattern

The captured file is raw sensor data in **Bayer GRBG** format:

```
Row 0: G R G R G R ...   (Green-Red alternating)
Row 1: B G B G B G ...   (Blue-Green alternating)
Row 2: G R G R G R ...
Row 3: B G B G B G ...
```

Each pixel captures only one color. Software must "demosaic" to interpolate full RGB.

## 6.3 Converting to Viewable Image

**Quick method (included in `capture.py`):**
- Extracts green channel for grayscale preview
- Applies brightness boost (images are dark by default)
- Outputs PNG via ImageMagick

**For full color demosaicing:**
- `rawtherapee`, `darktable` - GUI tools (set: GRBG, 8-bit, 3280×2464)
- `dcraw` - command line
- OpenCV `cv2.cvtColor(raw, cv2.COLOR_BAYER_GR2BGR)` - Python

---

# Part 7: Quick Reference

## Easy Method: Use the Scripts

```bash
cd /home/evans/linux-root/linux/chromeos-6.12/aidocs

# After reboot: load the patched camera module
sudo ./load_module.sh

# Capture photo (auto-detects devices, outputs /tmp/preview.ppm)
./capture.py

# Output PNG instead (requires ImageMagick)
./capture.py --png

# Capture with different brightness
./capture.py --gain HIGH   # Maximum gain (noisy but bright)
./capture.py --gain LOW    # Minimum gain (dark but clean)
./capture.py --gain MED    # Balanced (default)

# Adjust brightness boost (default: 4)
./capture.py --boost 2     # Less boost (darker)
./capture.py --boost 8     # More boost (brighter)

# Keep raw Bayer file too
./capture.py --raw         # Also saves /tmp/capture.raw
```

**capture.py features:**
- Auto-detects video device, media device, and sensor subdev
- Configures pipeline and sensor gain automatically
- Performs Bayer GRBG demosaicing to RGB
- Outputs PPM by default (no dependencies), PNG with `--png`

## Manual Pipeline Setup Commands

**Note:** Device paths vary between boots! Use `v4l2-ctl --list-devices` to find your actual paths, or just use `capture.py` which auto-detects everything.

```bash
# Find devices first
MEDIA_DEV=$(v4l2-ctl --list-devices | grep -A20 "mtk-cam-p1" | grep "/dev/media" | tr -d '[:space:]')
VIDEO_DEV=$(media-ctl -d $MEDIA_DEV -p | grep -A1 "main stream" | grep "video" | sed 's/.*\(\/dev\/video[0-9]*\).*/\1/')
SENSOR_DEV=$(media-ctl -d $MEDIA_DEV -p | grep -A1 "ov8856 2-0010" | grep "v4l-subdev" | sed 's/.*\(\/dev\/v4l-subdev[0-9]*\).*/\1/')

echo "Media: $MEDIA_DEV, Video: $VIDEO_DEV, Sensor: $SENSOR_DEV"

# 1. Enable sensor link
media-ctl -d $MEDIA_DEV -l "'ov8856 2-0010':0 -> '1a040000.seninf':0 [1]"

# 2. Set formats through the chain
media-ctl -d $MEDIA_DEV -V "'ov8856 2-0010':0 [fmt:SGRBG10_1X10/3280x2464]"
media-ctl -d $MEDIA_DEV -V "'1a040000.seninf':0 [fmt:SGRBG10_1X10/3280x2464]"
media-ctl -d $MEDIA_DEV -V "'1a040000.seninf':4 [fmt:SGRBG10_1X10/3280x2464]"

# 3. Set video output format (8-bit recommended)
v4l2-ctl -d $VIDEO_DEV --set-fmt-video=width=3280,height=2464,pixelformat='MBg8'

# 4. Set sensor gain (adjust for lighting)
v4l2-ctl -d $SENSOR_DEV --set-ctrl analogue_gain=1024
v4l2-ctl -d $SENSOR_DEV --set-ctrl digital_gain=2048
```

## After Reboot

The patched module needs to be reloaded:

```bash
cd /home/evans/linux-root/linux/chromeos-6.12/aidocs

# Use the load script (handles rmmod + insmod)
sudo ./load_module.sh

# Then capture - pipeline is auto-configured by capture.py
./capture.py
```

Or manually:

```bash
cd /home/evans/linux-root/linux/chromeos-6.12

# Rebuild module (if source changed)
make -j8 M=drivers/media/platform/mediatek/isp/isp_50/cam modules

# Load it
sudo rmmod mtk_cam_isp 2>/dev/null
sudo insmod drivers/media/platform/mediatek/isp/isp_50/cam/mtk-cam-isp.ko

# Verify
lsmod | grep mtk_cam
```

## Debugging Commands

```bash
# Find your media device first
MEDIA_DEV=$(v4l2-ctl --list-devices | grep -A20 "mtk-cam-p1" | grep "/dev/media" | tr -d '[:space:]')

# Check kernel messages
dmesg | tail -50

# Watch kernel messages live
sudo dmesg -w

# Check SCP firmware
dmesg | grep -i scp

# Check interrupt counts
cat /proc/interrupts | grep cam

# View media topology
media-ctl -d $MEDIA_DEV -p
```

---

# Part 8: Lessons Learned

1. **Complex cameras aren't plug-and-play.** You need to understand the pipeline and configure each component.

2. **Standard tools may not work.** `v4l2-ctl` assumes a simpler model. Advanced ISPs may require the Media Request API.

3. **Uninitialized variables are sneaky.** The `sd_fmt.pad` bug worked sometimes (when stack garbage was 0) and failed randomly.

4. **Kernel crashes are dangerous.** After an Oops, reboot immediately. Don't trust the system state.

5. **Read the driver source.** The comments and code tell you what's really expected.

---

# Part 9: Userland Camera Applications

## 9.1 Why Standard Camera Apps Don't Work

Apps like **GNOME Snapshot**, **Cheese**, or Firefox webcam access use this stack:
```
Application → PipeWire → libcamera → V4L2 device
```

**The problem:** libcamera has no pipeline handler for MT8183. Running `cam --list` shows:
```
$ cam --list
Available cameras:
(empty)
```

libcamera supports specific hardware with dedicated "pipeline handlers":
- Raspberry Pi (vc4, bcm2835)
- Intel IPU3/IPU6
- Rockchip (rkisp1)
- Simple pipeline (basic sensors)

**MT8183 is not supported** because:
1. The ISP requires the Media Request API (not standard V4L2 streaming)
2. MediaTek's MTISP format is proprietary
3. No one has written a pipeline handler for it

## 9.2 ChromeOS Camera Stack (Not Available)

On stock ChromeOS, cameras work via a completely different stack:
```
Chrome browser → Mojo IPC → Camera HAL → V4L2 + Request API
```

ChromeOS uses Android-style Camera HAL (Hardware Abstraction Layer) with proprietary 3A algorithms. This stack is not available on mainline Linux distributions.

## 9.3 Solution: Megapixels

**Megapixels** is a GTK4 camera app specifically designed for complex mobile cameras:

> "A GTK4 camera application that knows how to deal with the media request api."

It was created for postmarketOS and supports devices like PinePhone, Librem 5, and Pixel 3a.

### Installing Megapixels

Megapixels requires three components:
- **libmegapixels** - Device abstraction library
- **libdng** - DNG file writing
- **postprocessd** - Post-processing daemon
- **megapixels** - The GTK4 app itself

```bash
# Check if available in your distro
pacman -Ss megapixels  # Arch
apt search megapixels  # Debian/Ubuntu

# Or build from source:
# https://gitlab.com/megapixels-org/Megapixels
```

### Creating MT8183 Config

Megapixels needs a config file at `/usr/share/megapixels/config/google,krane.conf`:

```
Version = 1;
Make: "Google";
Model: "Krane Chromebook";

Rear: {
    SensorDriver: "ov8856";
    BridgeDriver: "mtk-cam-p1";
    IsoMin: 100;
    IsoMax: 64000;

    Modes: (
        {
            Width: 3280;
            Height: 2464;
            Rate: 15;
            Format: "GRBG8";
            Rotate: 0;
            FocalLength: 3.5;
            FNumber: 2.2;

            Pipeline: (
                {Type: "Link", From: "ov8856 2-0010", FromPad: 0, To: "1a040000.seninf", ToPad: 0},
                {Type: "Mode", Entity: "ov8856 2-0010"},
                {Type: "Mode", Entity: "1a040000.seninf"}
            );
        }
    );
};

Front: {
    SensorDriver: "ov02a10";
    BridgeDriver: "mtk-cam-p1";
    
    Modes: (
        {
            Width: 1600;
            Height: 1200;
            Rate: 30;
            Format: "GRBG8";
            Rotate: 0;
            Mirror: true;

            Pipeline: (
                {Type: "Link", From: "ov02a10 4-003d", FromPad: 0, To: "1a040000.seninf", ToPad: 0},
                {Type: "Mode", Entity: "ov02a10 4-003d"},
                {Type: "Mode", Entity: "1a040000.seninf"}
            );
        }
    );
};
```

**Note:** This config may need adjustments. The MT8183 ISP is more complex than PinePhone's simple CSI bridge, and Megapixels may need code changes to fully support Request API with SCP.

## 9.4 Alternative: v4l2loopback Virtual Webcam

If Megapixels doesn't work, create a **virtual webcam** that standard apps can use:

```
capture_camera.c → convert → ffmpeg → v4l2loopback → Apps see normal webcam
(Request API)      (Bayer→RGB)         (/dev/video9)
```

### Setup

```bash
# Load v4l2loopback module
sudo modprobe v4l2loopback video_nr=9 card_label="MT8183-Camera" exclusive_caps=1

# Verify it exists
v4l2-ctl --list-devices
# Should show: MT8183-Camera (/dev/video9)

# Feed frames to it (example using ffmpeg)
./capture_camera /dev/video1 /tmp/frame.raw && \
ffmpeg -f rawvideo -pix_fmt bayer_grbg8 -s 3280x2464 -i /tmp/frame.raw \
       -vf "scale=1280:720" -pix_fmt yuyv422 -f v4l2 /dev/video9
```

### Continuous Streaming Daemon

For real-time video, you'd need a daemon that:
1. Continuously captures frames using Request API
2. Debayers (Bayer → RGB)
3. Scales to reasonable resolution
4. Feeds to v4l2loopback in YUYV format

This is more complex but guarantees compatibility with any V4L2-aware application.

## 9.5 Status Summary

| Method | Status | Notes |
|--------|--------|-------|
| GNOME Snapshot | ❌ Won't work | Needs libcamera support |
| Firefox webcam | ❌ Won't work | Uses PipeWire → libcamera |
| Megapixels | 🔄 Testing | Needs config file, may need code changes |
| v4l2loopback | ✅ Workaround | Works but requires custom daemon |
| `capture.py` | ✅ Working | Command-line only, single shots |

---

# Appendix A: File Locations

| File | Purpose |
|------|---------|
| `drivers/media/platform/mediatek/isp/isp_50/cam/mtk_cam.c` | Main ISP driver (contains Bug #1) |
| `drivers/media/platform/mediatek/isp/isp_50/seninf/mtk_seninf.c` | Sensor interface driver |
| `drivers/media/i2c/ov8856.c` | Back camera sensor driver |
| `aidocs/capture_camera.c` | Low-level capture program (Request API) |
| `aidocs/capture.py` | Easy capture script (auto-detects devices, outputs PPM/PNG) |
| `aidocs/load_module.sh` | Loads the patched camera kernel module |
| `aidocs/init_camera.sh` | Pipeline initialization script (legacy, capture.py does this now) |

# Appendix B: Device Mapping

**Device numbers change between boots!** This is normal for Linux media devices.

`capture.py` auto-detects the correct devices by:
1. Finding the media device associated with `mtk-cam-p1`
2. Parsing the media topology to find "main stream" video device
3. Finding the sensor subdev (ov8856 or ov02a10)

### To check manually:
```bash
# Find the mtk-cam-p1 media device
v4l2-ctl --list-devices | grep -A20 "mtk-cam-p1"

# Example outputs from different boots:
# Boot 1: /dev/media0, /dev/video3, /dev/v4l-subdev2
# Boot 2: /dev/media1, /dev/video5, /dev/v4l-subdev3
```

### Entity to device mapping (use media-ctl to find current paths):
```bash
media-ctl -d /dev/media0 -p | grep -E "entity|device node"
```

| Entity | Typical Device | Purpose |
|--------|----------------|----------|
| `mtk-cam-p1 meta input` | /dev/video2-4 | Tuning parameters |
| `mtk-cam-p1 main stream` | /dev/video3-5 | **Main capture device** |
| `mtk-cam-p1 packed out` | /dev/video5-6 | Packed output |
| `ov8856 2-0010` | /dev/v4l-subdev2-3 | Back camera sensor controls |
| `ov02a10 4-003d` | /dev/v4l-subdev3-4 | Front camera sensor controls |

# Appendix C: Common Errors

| Error | Meaning | Solution |
|-------|---------|----------|
| `-EPIPE` (-32) | Format mismatch in pipeline | Check all pad formats match |
| `-EBUSY` (-16) | Device in use or bad state | Reboot |
| `-EINVAL` (-22) | Invalid parameter | Check format, pad number |
| Hang on capture | SCP not processing | Use Request API, not standard streaming |

---

*Document created during camera debugging session, 2026-03-04*  
*Updated 2026-03-05: Scripts now auto-detect devices, output PPM by default*
