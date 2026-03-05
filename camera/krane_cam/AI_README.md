# krane_cam - MT8183 Camera Service

A Rust implementation of camera capture for the MT8183 (Kukui Krane) Chromebook, feeding frames to a v4l2loopback virtual camera device.

## Overview

The MT8183 camera subsystem requires the **Media Request API** for capture—standard V4L2 streaming doesn't work because the ISP uses a System Control Processor (SCP) for frame processing. This service handles that complexity and presents a standard webcam interface via v4l2loopback.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│              User Application (Firefox, etc.)       │
│                         ↓                           │
│              /dev/video12 (v4l2loopback)            │
│                         ↑                           │
│                    krane_cam                        │
│     ┌───────────────────┼───────────────────┐       │
│     │ BayerFrame ──────→ YuyvFrame          │       │
│     │   (GRBG 8-bit)      (demosaic)        │       │
│     └───────────────────┼───────────────────┘       │
│                         ↓                           │
│           Media Request API (capture.rs)            │
│                         ↓                           │
│     /dev/video1 (ISP)  +  /dev/media0 (requests)    │
│                         ↓                           │
│              Kernel: mtk-cam-p1 driver              │
│                         ↓                           │
│              OV8856 sensor (3280×2464)              │
└─────────────────────────────────────────────────────┘
```

## File Structure

| File | Purpose |
|------|---------|
| `main.rs` | CLI entry point, frame loop, test pattern mode |
| `camera.rs` | High-level `Camera` struct, pipeline setup via shell commands |
| `capture.rs` | Low-level V4L2 + Media Request API ioctls, `CaptureSession` |
| `frame.rs` | `BayerFrame`, `YuyvFrame`, `BayerPattern` types with conversion |
| `loopback.rs` | `LoopbackDevice` for v4l2loopback output |
| `media.rs` | Device discovery (`CameraDevices`) |
| `pattern.rs` | `ColorBars` test pattern generator |

## Key Types

### `BayerFrame`
Raw Bayer data from the camera sensor.
- Dimensions: 3280×2464 (OV8856)
- Format: GRBG 8-bit (1 byte/pixel)
- Method: `to_yuyv()` → demosaics to half-resolution YUYV

### `YuyvFrame`
Converted frame ready for v4l2loopback.
- Dimensions: 1640×1232 (half of sensor)
- Format: YUYV 4:2:2 (2 bytes/pixel)

### `CaptureSession`
Manages V4L2 buffer lifecycle and Media Request API flow.
- Keeps streaming active between captures (optimization)
- Rotates through 4 kernel buffers
- Handles request allocation/queue/dequeue cycle

## Media Request API Flow

Each frame capture:
1. `MEDIA_IOC_REQUEST_ALLOC` → get request FD
2. `VIDIOC_QBUF` with `V4L2_BUF_FLAG_REQUEST_FD` → queue buffer to request
3. `MEDIA_REQUEST_IOC_QUEUE` → submit request to ISP
4. `select()` on request FD → wait for completion
5. `VIDIOC_DQBUF` → dequeue filled buffer
6. `MEDIA_REQUEST_IOC_REINIT` + close → cleanup request

Streaming (`VIDIOC_STREAMON`) is started once and kept active for performance.

## Usage

```bash
# Load v4l2loopback
sudo modprobe v4l2loopback video_nr=12 card_label="MT8183 Camera" exclusive_caps=1

# Run with real camera
cargo run -- -d /dev/video12 --camera

# Run with test pattern (no hardware needed)
cargo run -- -d /dev/video12 --width 640 --height 480
```

## Current Status (March 2026)

✅ **Working:**
- Device discovery (auto-detects media/video nodes)
- Pipeline configuration via media-ctl/v4l2-ctl
- Media Request API capture in pure Rust
- Bayer→YUYV demosaicing
- v4l2loopback output
- Continuous streaming optimization (~5 FPS in release mode)
- Successfully tested: 270+ frames captured at 1640×1232

**Performance:**
- Release build: ~5 FPS at 1640×1232 (half sensor resolution)
- Debug build: slower due to unoptimized demosaic

⚠️ **Limitations:**
- Rear camera (OV8856) only—front camera (OV02A10) not tested
- No auto-exposure/auto-white-balance (raw sensor output)
- Frame rate limited by ISP processing and Request API overhead
- Half-resolution output (demosaic halves dimensions)

**Future improvements:**
- Investigate parallel request queuing for higher FPS
- Add basic auto-exposure
- Test front camera support
- Consider SIMD optimization for demosaic

## ioctl Values

Correctly computed using `_IOC` formula:

| ioctl | Value | Notes |
|-------|-------|-------|
| `MEDIA_IOC_REQUEST_ALLOC` | `0x80047c05` | type='|', nr=5 |
| `MEDIA_REQUEST_IOC_QUEUE` | `0x00007c80` | type='|', nr=0x80 |
| `MEDIA_REQUEST_IOC_REINIT` | `0x00007c81` | type='|', nr=0x81 |
| `VIDIOC_REQBUFS` | `0xc0145608` | type='V', nr=8 |
| `VIDIOC_QBUF` | `0xc058560f` | type='V', nr=15 |
| `VIDIOC_DQBUF` | `0xc0585611` | type='V', nr=17 |

## Dependencies

- `v4l` crate (for loopback device)
- `libc` (raw ioctls for capture)
- `anyhow` (error handling)
- `clap` (CLI)

## References

- [Media Request API docs](https://www.kernel.org/doc/html/latest/userspace-api/media/mediactl/request-api.html)
- [v4l2loopback](https://github.com/umlaeute/v4l2loopback)
- Original C implementation: `../camera_utils/capture_camera.c`
