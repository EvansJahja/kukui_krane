# MT8183 ISP and Hardware Acceleration Investigation

**Date:** March 6, 2026  
**Status:** Complete exploration of hardware capabilities

---

## Executive Summary

The MT8183 ISP alone only outputs raw Bayer formats, but the SoC includes a complete **hardware acceleration pipeline** for image processing that we're not currently using. Key discovery: dedicated JPEG encoder, video encoder, format converter, and face detection blocks.

**Performance Opportunity:** Instead of CPU-based Bayer demosaicing → YUYV conversion, we could potentially use:
```
Raw Bayer (ISP) → MDP3 Format Converter → JPEG Encoder → Hardware JPEG
```

---

## Hardware Block Discovery

### 1. Camera ISP (`mtk-cam-p1`)
**Media Device:** `/dev/media0`

| Device | Purpose | Formats | Status |
|--------|---------|---------|--------|
| `/dev/video1` | Main stream | Bayer variants (MBg8, MBgA, MBgC, MBgE) | ✅ Currently used |
| `/dev/video2` | Packed out | Full-G Bayer variants (MFg8, MFgA, etc.) | 🔍 Unexplored |
| `/dev/video0` | Meta input | Tuning parameters | ⭕ Metadata only |
| `/dev/video3-6` | Partial meta | Processing metadata | ⭕ Metadata only |

**ISP Capabilities:**
- ✅ 8-bit to 14-bit Bayer output
- ✅ GRBG, GBRG, BGGR, RGGB Bayer patterns
- ❌ No YUV output directly from ISP
- ❌ No JPEG output directly from ISP
- ❌ No RGB output directly from ISP

### 2. Hardware JPEG Encoder (`mtk-jpeg-enc`)
**Device:** `/dev/video10`  
**Type:** Memory-to-Memory Multiplanar

**Input Formats:**
- `NM12` - Y/UV 4:2:0 (N-C)
- `NM21` - Y/VU 4:2:0 (N-C)  
- `YUYV` - YUYV 4:2:2
- `YVYU` - YVYU 4:2:2

**Output Format:**
- `JPEG` - JFIF JPEG, compressed

**🔥 Key Insight:** If we can convert Bayer → YUV, this hardware can produce JPEG directly!

### 3. Hardware Video Encoder (`MT8183 video encoder`)
**Device:** `/dev/video9`  
**Type:** Memory-to-Memory Multiplanar

**Input Formats:**
- `NM12` - Y/UV 4:2:0 (N-C)
- `NM21` - Y/VU 4:2:0 (N-C)
- `YM12` - Planar YUV 4:2:0 (N-C)
- `YM21` - Planar YVU 4:2:0 (N-C)

**Output Format:**
- `H264` - H.264 compressed video

### 4. MediaTek MDP3 (`MediaTek MDP3`)
**Device:** `/dev/video8`  
**Type:** Memory-to-Memory Multiplanar  
**Purpose:** Format converter and scaler

**Input Formats (26 total):**
```
RGB:   GREY, RGBR, RGBP, RGB3, BGR3, AR24, BA24
YUV:   UYVY, VYUY, YUYV, YVYU, YU12, YV12, NV12, NV21, NV16, NV61, NV24, NV42
MediaTek: MT21, MM21, NM12, NM21, NM16, NM61, YM12, YM21
```

**Output Formats (18 total):**
```
RGB:   GREY, RGBR, RGBP, RGB3, BGR3, AR24, BA24
YUV:   UYVY, VYUY, YUYV, YVYU, YU12, YV12, NV12, NV21
MediaTek: NM12, NM21, YM12, YM21
```

**🔥 Key Insight:** This is the missing link! MDP3 can convert between many formats and scale images.

### 5. Face Detection Hardware (`mtk-fd-4.0`)
**Device:** `/dev/video7`  
**Media Device:** `/dev/media1`

Face detection pipeline with source → proc → sink topology. Potential for real-time face detection overlays.

### 6. Video Decoder (`MT8183 video decoder`)
**Device:** `/dev/video11`  
**Media Device:** `/dev/media2`

For H.264/video decoding (not directly relevant to camera capture).

---

## Current vs. Potential Pipeline

### Current Implementation
```
OV8856 Sensor (SGRBG10) 
    ↓
SENINF 
    ↓  
ISP (SCP Request API)
    ↓
/dev/video1 (MBg8 - 8-bit Bayer GRBG)
    ↓
CPU Demosaic (Rust) → YUYV
    ↓
v4l2loopback (/dev/video12)
```

**Performance:** ~5 FPS at 1640×1232

### Potential Hardware-Accelerated Pipeline

#### Option A: Hardware JPEG Output
```
ISP → MBg8 Bayer → MDP3 Converter → YUYV → JPEG Encoder → JPEG file
    /dev/video1      /dev/video8       /dev/video10
```

#### Option B: Hardware YUV for Streaming  
```
ISP → MBg8 Bayer → MDP3 Converter → YUYV → v4l2loopback
    /dev/video1      /dev/video8       direct feed
```

#### Option C: Multiple Concurrent Streams
```
ISP → MBg8 Bayer ┬→ MDP3 → YUYV → loopback (preview)
                  └→ MDP3 → YUV → JPEG (stills)
```

---

## Format Analysis

### ISP "Packed Out" Device (`/dev/video2`)
Discovered **"Full-G Bayer"** formats vs. standard Bayer:

| Standard Bayer | Full-G Bayer | Notes |
|---------------|--------------|--------|
| `MBg8` | `MFg8` | Same bit depth, different packing? |
| `MBgA` | `MFgA` | 10-bit versions |

**Question:** What's the difference between standard and "Full-G" Bayer? Potentially different ISP processing modes.

### MediaTek-Specific Formats
- `MT21` - MediaTek Compressed Format
- `MM21` - MediaTek 8-bit Block Format  
- `NM12/NM21` - Y/UV formats with "(N-C)" annotation
- `YM12/YM21` - Planar YUV formats with "(N-C)" annotation

The "(N-C)" likely means "Non-Cached" or a specific memory layout for hardware acceleration.

---

## Investigation Gaps

### Unexplored Areas
1. **Packed Out Device:** Never tested `/dev/video2` capture with "Full-G" formats
2. **MDP3 Bayer Input:** Can MDP3 accept raw Bayer formats directly?
3. **Format Chain Compatibility:** Which format sequences actually work in practice?
4. **Face Detection Pipeline:** Could provide real-time face detection overlays
5. **Multiple Device Coordination:** Can we capture from multiple devices simultaneously?

### Critical Questions
1. **Bayer → MDP3:** Can MDP3 accept raw Bayer as input, or only processed YUV/RGB?
2. **Memory-to-Memory:** How do we feed ISP output to MDP3 input efficiently?
3. **Request API Compatibility:** Do hardware encoders work with Media Request API?
4. **Performance Reality:** Would hardware pipeline actually be faster than current CPU approach?

---

## Implementation Strategy

### Phase 1: Format Compatibility Testing
1. Test if MDP3 can accept Bayer formats as input
2. Test Bayer → YUYV conversion through MDP3
3. Compare processing speed: CPU demosaic vs. MDP3 conversion

### Phase 2: Hardware JPEG Pipeline  
1. Implement Bayer → MDP3 → JPEG encoder chain
2. Measure performance vs. current Bayer → CPU → YUYV path
3. Compare JPEG quality and file sizes

### Phase 3: Streaming Optimization
1. Test MDP3 output directly to v4l2loopback
2. Implement concurrent streams (preview + stills)
3. Add face detection overlay integration

### Phase 4: Advanced Features
1. Test "Full-G Bayer" formats from `/dev/video2`
2. Multiple resolution outputs
3. Hardware-accelerated format conversion for different applications

---

## Hardware Architecture Summary

The MT8183 is not just a simple ISP - it's a **complete camera SoC** with:

```
┌─────────────┐    ┌──────────┐    ┌─────────────┐
│   Sensor    │───▶│   ISP    │───▶│ Raw Bayer   │
│   (OV8856)  │    │(mtk-cam) │    │ /dev/video1 │
└─────────────┘    └──────────┘    └─────────────┘
                                            │
                                            ▼
┌─────────────┐    ┌──────────┐    ┌─────────────┐
│Face Detect  │    │   MDP3   │    │  Hardware   │
│mtk-fd-4.0   │    │Converter │    │JPEG Encoder │  
│/dev/video7  │    │/dev/video8│   │/dev/video10 │
└─────────────┘    └──────────┘    └─────────────┘
```

**Current Status:** We're only using the ISP block and doing everything else in software. There's significant hardware acceleration available that we haven't tapped into.

---

## Next Steps

1. **Test MDP3 Bayer compatibility** - Can it accept raw Bayer input?
2. **Benchmark hardware pipeline** - Measure actual performance improvements
3. **Implement hardware JPEG** - Replace CPU-intensive demosaicing
4. **Add to krane_cam** - Integrate hardware-accelerated options

The discovery of this hardware pipeline could potentially **multiply frame rate performance** and reduce CPU usage dramatically.