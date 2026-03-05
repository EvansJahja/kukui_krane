#!/usr/bin/env python3
"""
MT8183 Camera Capture Script
Captures a raw Bayer image and converts to viewable image

Auto-detects camera devices by querying v4l2-ctl and media-ctl.

Usage: ./capture.py [--gain LOW|MED|HIGH] [--raw] [--png] [--boost N]
  --gain:  Set sensor gain (LOW=dark, MED=balanced, HIGH=bright)
  --raw:   Also save raw Bayer file to /tmp/capture.raw
  --png:   Output PNG instead of PPM (requires ImageMagick)
  --boost: Brightness boost factor for preview (default: 4)
"""

import subprocess
import sys
import os
import re
import argparse
import numpy as np
from dataclasses import dataclass
from typing import Optional, Dict, Tuple


# ============================================================================
# Configuration
# ============================================================================

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
CAPTURE_PROG = os.path.join(SCRIPT_DIR, "capture_camera")
RAW_FILE = "/tmp/capture.raw"
OUTPUT_PPM = "/tmp/preview.ppm"
OUTPUT_PNG = "/tmp/preview.png"

# Image dimensions for OV8856 back camera
WIDTH = 3280
HEIGHT = 2464

# Gain presets
GAIN_PRESETS = {
    "LOW":  {"analogue_gain": 128,  "digital_gain": 1024},   # Dark, low noise
    "MED":  {"analogue_gain": 1024, "digital_gain": 2048},   # Balanced
    "HIGH": {"analogue_gain": 2047, "digital_gain": 4095},   # Max brightness, noisy
}


# ============================================================================
# Data Classes
# ============================================================================

@dataclass
class CameraDevices:
    """Holds discovered camera device paths."""
    video_device: str          # Main capture device (e.g., /dev/video5)
    media_device: str          # Media controller (e.g., /dev/media0)
    sensor_subdev: str         # Sensor subdev for controls (e.g., /dev/v4l-subdev3)
    sensor_name: str           # Sensor entity name (e.g., "ov8856 2-0010")


# ============================================================================
# Device Discovery Functions
# ============================================================================

def run_command(cmd: list, check: bool = True) -> subprocess.CompletedProcess:
    """Run a command and return the result."""
    return subprocess.run(cmd, capture_output=True, text=True, check=check)


def find_isp_media_device() -> Optional[str]:
    """
    Find the media device for mtk-cam-p1 ISP.
    Parses 'v4l2-ctl --list-devices' output.
    
    Returns: Path like '/dev/media0' or None if not found.
    """
    result = run_command(["v4l2-ctl", "--list-devices"], check=False)
    if result.returncode != 0:
        return None
    
    lines = result.stdout.split('\n')
    in_mtk_cam_section = False
    
    for line in lines:
        if 'mtk-cam-p1' in line:
            in_mtk_cam_section = True
            continue
        if in_mtk_cam_section:
            line = line.strip()
            if line.startswith('/dev/media'):
                return line
            if line and not line.startswith('/dev/'):
                # New device section started
                in_mtk_cam_section = False
    
    return None


def find_main_stream_device(media_device: str) -> Optional[str]:
    """
    Find the main stream video device from media topology.
    Parses 'media-ctl -p' output looking for 'main stream' entity.
    
    Returns: Path like '/dev/video5' or None if not found.
    """
    result = run_command(["media-ctl", "-d", media_device, "-p"], check=False)
    if result.returncode != 0:
        return None
    
    lines = result.stdout.split('\n')
    found_main_stream = False
    
    for line in lines:
        # Look for entity line containing "main stream"
        if 'entity' in line and 'main stream' in line:
            found_main_stream = True
            continue
        
        # After finding main stream, get the device node
        if found_main_stream and 'device node name' in line:
            match = re.search(r'/dev/video\d+', line)
            if match:
                return match.group(0)
            found_main_stream = False
        
        # Reset if we hit another entity
        if found_main_stream and line.strip().startswith('- entity'):
            found_main_stream = False
    
    return None


def find_sensor_info(media_device: str) -> Optional[Tuple[str, str]]:
    """
    Find the active sensor's subdev and entity name.
    Looks for enabled sensor link (ov8856 or ov02a10).
    
    Returns: Tuple of (subdev_path, entity_name) or None.
    """
    result = run_command(["media-ctl", "-d", media_device, "-p"], check=False)
    if result.returncode != 0:
        return None
    
    lines = result.stdout.split('\n')
    current_entity = None
    current_subdev = None
    
    for i, line in enumerate(lines):
        # Look for sensor entities
        if 'entity' in line and ('ov8856' in line or 'ov02a10' in line):
            # Extract entity name like "ov8856 2-0010"
            match = re.search(r'entity \d+: (ov\w+ \d+-\d+)', line)
            if match:
                current_entity = match.group(1)
        
        # Get the subdev path
        if current_entity and 'device node name' in line:
            match = re.search(r'/dev/v4l-subdev\d+', line)
            if match:
                current_subdev = match.group(0)
        
        # Check if this sensor has an ENABLED link
        if current_entity and current_subdev and 'ENABLED' in line and 'seninf' in line:
            return (current_subdev, current_entity)
        
        # Reset if we hit a new entity
        if line.strip().startswith('- entity') and current_entity:
            if 'ov8856' not in line and 'ov02a10' not in line:
                current_entity = None
                current_subdev = None
    
    # If no enabled link found, return first sensor found (ov8856 preferred)
    # Re-scan for ov8856
    current_entity = None
    current_subdev = None
    for line in lines:
        if 'entity' in line and 'ov8856' in line:
            match = re.search(r'entity \d+: (ov\w+ \d+-\d+)', line)
            if match:
                current_entity = match.group(1)
        if current_entity and 'device node name' in line:
            match = re.search(r'/dev/v4l-subdev\d+', line)
            if match:
                return (match.group(0), current_entity)
    
    return None


def discover_devices() -> CameraDevices:
    """
    Auto-discover all camera devices.
    
    Returns: CameraDevices with all paths populated.
    Raises: SystemExit if devices cannot be found.
    """
    print("Discovering camera devices...")
    
    # Find media device
    media_device = find_isp_media_device()
    if not media_device:
        print("ERROR: Could not find mtk-cam-p1 media device.")
        print("Is the camera module loaded? Try: sudo insmod mtk-cam-isp.ko")
        sys.exit(1)
    print(f"  Media device: {media_device}")
    
    # Find main stream video device
    video_device = find_main_stream_device(media_device)
    if not video_device:
        print("ERROR: Could not find main stream video device.")
        sys.exit(1)
    print(f"  Video device: {video_device}")
    
    # Find sensor
    sensor_info = find_sensor_info(media_device)
    if not sensor_info:
        print("ERROR: Could not find camera sensor (ov8856 or ov02a10).")
        sys.exit(1)
    sensor_subdev, sensor_name = sensor_info
    print(f"  Sensor: {sensor_name} ({sensor_subdev})")
    
    return CameraDevices(
        video_device=video_device,
        media_device=media_device,
        sensor_subdev=sensor_subdev,
        sensor_name=sensor_name
    )


# ============================================================================
# Pipeline Configuration Functions
# ============================================================================

def enable_sensor_link(devices: CameraDevices) -> bool:
    """
    Enable the link from sensor to SENINF.
    
    Returns: True on success.
    """
    # Determine which SENINF pad based on sensor
    seninf_pad = "0" if "ov8856" in devices.sensor_name else "1"
    
    link_cmd = f"'{devices.sensor_name}':0 -> '1a040000.seninf':{seninf_pad} [1]"
    result = run_command([
        "media-ctl", "-d", devices.media_device, "-l", link_cmd
    ], check=False)
    
    return result.returncode == 0


def configure_pipeline_formats(devices: CameraDevices) -> bool:
    """
    Configure formats for all pads in the pipeline.
    
    The sensor outputs 10-bit Bayer, which flows through SENINF to ISP.
    Pipeline: sensor -> seninf:0 (sink) -> seninf:4 (source) -> ISP
    
    Returns: True if all configurations succeeded.
    """
    # Determine format based on sensor
    if "ov8856" in devices.sensor_name:
        fmt = "SGRBG10_1X10"
        width, height = 3280, 2464
        seninf_sink = "0"
    else:  # ov02a10
        fmt = "SRGGB10_1X10"
        width, height = 1600, 1200
        seninf_sink = "1"
    
    success = True
    
    # 1. Set sensor output format
    result = run_command([
        "media-ctl", "-d", devices.media_device, "-V",
        f"'{devices.sensor_name}':0 [fmt:{fmt}/{width}x{height}]"
    ], check=False)
    if result.returncode != 0:
        success = False
    
    # 2. Set SENINF sink pad format (must match sensor)
    result = run_command([
        "media-ctl", "-d", devices.media_device, "-V",
        f"'1a040000.seninf':{seninf_sink} [fmt:{fmt}/{width}x{height}]"
    ], check=False)
    if result.returncode != 0:
        success = False
    
    # 3. Set SENINF source pad format (pad 4 goes to ISP)
    result = run_command([
        "media-ctl", "-d", devices.media_device, "-V",
        f"'1a040000.seninf':4 [fmt:{fmt}/{width}x{height}]"
    ], check=False)
    if result.returncode != 0:
        success = False
    
    return success


def configure_video_format(devices: CameraDevices) -> bool:
    """
    Set the video capture format to 8-bit Bayer GRBG.
    
    Returns: True on success.
    """
    result = run_command([
        "v4l2-ctl", "-d", devices.video_device,
        "--set-fmt-video", f"width={WIDTH},height={HEIGHT},pixelformat=MBg8"
    ], check=False)
    
    return result.returncode == 0


def set_sensor_exposure(devices: CameraDevices, exposure: int = 2482) -> bool:
    """
    Set sensor exposure time.
    
    Returns: True on success.
    """
    result = run_command([
        "v4l2-ctl", "-d", devices.sensor_subdev,
        "--set-ctrl", f"exposure={exposure}"
    ], check=False)
    
    return result.returncode == 0


def set_sensor_gain(devices: CameraDevices, preset: str) -> Dict[str, int]:
    """
    Set sensor gain to a preset level.
    
    Returns: Dictionary with applied gain values.
    """
    gains = GAIN_PRESETS.get(preset.upper(), GAIN_PRESETS["MED"])
    
    run_command([
        "v4l2-ctl", "-d", devices.sensor_subdev,
        "--set-ctrl", f"analogue_gain={gains['analogue_gain']}"
    ], check=False)
    
    run_command([
        "v4l2-ctl", "-d", devices.sensor_subdev,
        "--set-ctrl", f"digital_gain={gains['digital_gain']}"
    ], check=False)
    
    return gains


# ============================================================================
# Capture Functions
# ============================================================================

def check_capture_program() -> None:
    """Verify capture_camera binary exists, exit if not."""
    if not os.path.exists(CAPTURE_PROG):
        print(f"ERROR: {CAPTURE_PROG} not found.")
        print(f"Compile with: gcc -o capture_camera capture_camera.c -Wall")
        sys.exit(1)


def capture_raw_frame(devices: CameraDevices) -> str:
    """
    Capture a raw Bayer frame using capture_camera program.
    
    Returns: Path to raw file.
    Raises: SystemExit on failure.
    """
    result = subprocess.run(
        [CAPTURE_PROG, devices.video_device, devices.media_device, RAW_FILE],
        capture_output=True, text=True
    )
    
    if result.returncode != 0 or not os.path.exists(RAW_FILE):
        print("Capture failed!")
        print(result.stdout)
        print(result.stderr)
        sys.exit(1)
    
    return RAW_FILE


# ============================================================================
# Image Conversion Functions
# ============================================================================

def demosaic_bayer_grbg(raw_data: bytes, width: int, height: int) -> np.ndarray:
    """
    Fast demosaic of Bayer GRBG pattern to RGB using numpy.
    
    Bayer pattern (GRBG):
      Row 0: G R G R G R ...  (even rows: G at even cols, R at odd cols)
      Row 1: B G B G B G ...  (odd rows: B at even cols, G at odd cols)
    
    Uses simple 2x2 block extraction for speed.
    Output is half resolution.
    
    Returns: RGB numpy array of shape (height//2, width//2, 3).
    """
    # Convert to numpy array
    raw = np.frombuffer(raw_data, dtype=np.uint8).reshape(height, width)
    
    # Extract each color from the 2x2 Bayer blocks
    # GRBG pattern:
    #   [G][R]  <- even row
    #   [B][G]  <- odd row
    g1 = raw[0::2, 0::2]  # Green at (even row, even col)
    r  = raw[0::2, 1::2]  # Red at (even row, odd col)
    b  = raw[1::2, 0::2]  # Blue at (odd row, even col)
    g2 = raw[1::2, 1::2]  # Green at (odd row, odd col)
    
    # Average the two green channels
    g = ((g1.astype(np.uint16) + g2.astype(np.uint16)) // 2).astype(np.uint8)
    
    # Stack into RGB
    rgb = np.stack([r, g, b], axis=-1)
    
    return rgb


def apply_brightness_boost_rgb(image: np.ndarray, boost: int) -> np.ndarray:
    """Apply brightness boost to RGB image using numpy."""
    boosted = image.astype(np.uint16) * boost
    return np.clip(boosted, 0, 255).astype(np.uint8)


def save_ppm(rgb_data: np.ndarray, filepath: str) -> None:
    """Save RGB numpy array as PPM (P6) file."""
    height, width = rgb_data.shape[:2]
    with open(filepath, 'wb') as f:
        f.write(f'P6\n{width} {height}\n255\n'.encode())
        f.write(rgb_data.tobytes())


def convert_ppm_to_png(ppm_file: str, png_file: str) -> bool:
    """
    Convert PPM to PNG using ImageMagick.
    
    Returns: True on success.
    """
    result = run_command(["convert", ppm_file, png_file], check=False)
    return result.returncode == 0


def raw_to_image(raw_file: str, output_file: str, brightness_boost: int = 4, 
                 use_png: bool = False) -> str:
    """
    Convert 8-bit Bayer GRBG raw to viewable color image.
    
    Performs demosaicing using numpy for fast processing.
    
    Args:
        raw_file: Path to raw Bayer file
        output_file: Path for output image
        brightness_boost: Multiplier for brightness
        use_png: If True, output PNG (requires ImageMagick); otherwise PPM
    
    Returns: Path to output file.
    """
    with open(raw_file, 'rb') as f:
        raw_data = f.read()
    
    expected_size = WIDTH * HEIGHT
    if len(raw_data) != expected_size:
        print(f"WARNING: Raw file size {len(raw_data)} != expected {expected_size}")
    
    # Demosaic Bayer to RGB (half resolution)
    rgb = demosaic_bayer_grbg(raw_data, WIDTH, HEIGHT)
    rgb = apply_brightness_boost_rgb(rgb, brightness_boost)
    
    if use_png:
        # Save as PPM then convert to PNG
        ppm_file = "/tmp/preview_temp.ppm"
        save_ppm(rgb, ppm_file)
        
        if convert_ppm_to_png(ppm_file, output_file):
            os.remove(ppm_file)
            return output_file
        else:
            print(f"ImageMagick conversion failed. PPM saved to {ppm_file}")
            return ppm_file
    else:
        # Save directly as PPM (no dependencies)
        save_ppm(rgb, output_file)
        return output_file


# ============================================================================
# Main Entry Point
# ============================================================================

def parse_args() -> argparse.Namespace:
    """Parse command line arguments."""
    parser = argparse.ArgumentParser(
        description="Capture photo from MT8183 camera (auto-detects devices)"
    )
    parser.add_argument(
        "--gain", choices=["LOW", "MED", "HIGH"], default="MED",
        help="Sensor gain preset (default: MED)"
    )
    parser.add_argument(
        "--raw", action="store_true",
        help="Keep raw Bayer file at /tmp/capture.raw"
    )
    parser.add_argument(
        "--png", action="store_true",
        help="Output PNG instead of PPM (requires ImageMagick)"
    )
    parser.add_argument(
        "--boost", type=int, default=1,
        help="Brightness boost factor for preview (default: 1)"
    )
    return parser.parse_args()


def main() -> None:
    """Main capture workflow."""
    args = parse_args()
    
    print("=== MT8183 Camera Capture ===\n")
    
    # Check prerequisites
    check_capture_program()
    
    # Discover devices
    devices = discover_devices()
    print()
    
    # Configure pipeline (full initialization)
    print("Configuring pipeline...")
    print("  Enabling sensor link...")
    enable_sensor_link(devices)
    print("  Setting pad formats...")
    configure_pipeline_formats(devices)
    print("  Setting video format...")
    configure_video_format(devices)
    print("  Setting exposure...")
    set_sensor_exposure(devices)
    
    # Set sensor gain
    gains = set_sensor_gain(devices, args.gain)
    print(f"  Gain: {args.gain} (analogue={gains['analogue_gain']}, digital={gains['digital_gain']})")
    print()
    
    # Capture
    print("Capturing frame...")
    raw_file = capture_raw_frame(devices)
    raw_size = os.path.getsize(raw_file)
    print(f"  Captured {raw_size} bytes")
    
    # Convert to viewable format
    output_file = OUTPUT_PNG if args.png else OUTPUT_PPM
    fmt_name = "PNG" if args.png else "PPM"
    print(f"Converting to {fmt_name}...")
    output = raw_to_image(raw_file, output_file, args.boost, use_png=args.png)
    
    # Cleanup raw file unless --raw specified
    if not args.raw and os.path.exists(RAW_FILE):
        os.remove(RAW_FILE)
    
    # Summary
    print(f"\nOutput: {output}")
    if args.raw:
        print(f"Raw file: {RAW_FILE}")
    
    if os.path.exists(output):
        size = os.path.getsize(output)
        print(f"{fmt_name} size: {size / 1024:.1f} KB")


if __name__ == "__main__":
    main()
