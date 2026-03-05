//! V4L2 loopback device handling

use anyhow::{Context, Result};
use std::io::Write;
use v4l::video::Output;
use v4l::Device;
use v4l::FourCC;

use crate::frame::YuyvFrame;

/// Represents a v4l2loopback output device
pub struct LoopbackDevice {
    device: Device,
    width: u32,
    height: u32,
    frame_size: usize,
}

impl LoopbackDevice {
    /// Open and configure a v4l2loopback device
    pub fn open(path: &str, width: u32, height: u32) -> Result<Self> {
        println!("Opening device {}...", path);
        let device = Device::with_path(path)
            .context("Failed to open device")?;

        // Check capabilities
        let caps = device.query_caps().context("Failed to query device capabilities")?;
        println!("Device capabilities:");
        println!("  Driver: {}", caps.driver);
        println!("  Card: {}", caps.card);
        println!("  Capabilities: {:?}", caps.capabilities);
        println!();

        // Set format to YUYV
        let mut fmt = device.format().context("Failed to read current format")?;
        fmt.width = width;
        fmt.height = height;
        fmt.fourcc = FourCC::new(b"YUYV");
        
        let fmt = device.set_format(&fmt).context("Failed to set format")?;
        println!("Format set successfully:");
        println!("  FourCC: {}", fmt.fourcc);
        println!("  Size: {}x{}", fmt.width, fmt.height);
        println!();

        let frame_size = (fmt.width * fmt.height * 2) as usize; // YUYV is 2 bytes per pixel

        Ok(Self {
            device,
            width: fmt.width,
            height: fmt.height,
            frame_size,
        })
    }

    /// Write a YUYV frame to the loopback device
    pub fn write_frame(&mut self, frame: &YuyvFrame) -> Result<usize> {
        if frame.width() != self.width || frame.height() != self.height {
            anyhow::bail!(
                "Frame dimensions mismatch: expected {}x{}, got {}x{}",
                self.width, self.height, frame.width(), frame.height()
            );
        }

        if frame.byte_len() != self.frame_size {
            anyhow::bail!(
                "Frame size mismatch: expected {} bytes, got {}",
                self.frame_size,
                frame.byte_len()
            );
        }

        self.device
            .write(frame.data())
            .context("Failed to write frame")
    }

    /// Write raw frame data (for test patterns)
    pub fn write_raw(&mut self, data: &[u8]) -> Result<usize> {
        if data.len() != self.frame_size {
            anyhow::bail!(
                "Frame size mismatch: expected {} bytes, got {}",
                self.frame_size,
                data.len()
            );
        }

        self.device
            .write(data)
            .context("Failed to write frame")
    }

    /// Get the configured width
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get the configured height
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get the expected frame size in bytes
    pub fn frame_size(&self) -> usize {
        self.frame_size
    }
}
