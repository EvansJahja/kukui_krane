//! Camera capture using MT8183 ISP and Media Request API

use anyhow::{Context, Result};
use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::process::Command;

use crate::capture::CaptureSession;
use crate::frame::BayerFrame;
use crate::media::{self, CameraDevices};

/// MT8183 Camera for request-based capture
pub struct Camera {
    devices: CameraDevices,
    _video_device: File, // Keep file open for its lifetime
    _media_device: File,
    capture_session: CaptureSession,
    width: u32,
    height: u32,
}

impl Camera {
    /// Discover and open the MT8183 camera
    pub fn open() -> Result<Self> {
        // Discover devices
        let devices = media::discover()?;
        
        // Validate OV8856 (rear camera) for MVP
        if !devices.sensor_name.contains("ov8856") {
            anyhow::bail!(
                "MVP supports OV8856 only. Found: {}",
                devices.sensor_name
            );
        }
        
        // Open video device as File for direct ioctl access (read+write)
        let video_device = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&devices.video_device)
            .with_context(|| format!("Failed to open video device: {}", devices.video_device.display()))?;
        
        // Open media device for request allocation
        let media_device = media::open_media_device(&devices.media_device)?;
        
        // OV8856 dimensions
        let width = 3280u32;
        let height = 2464u32;
        
        println!("\nCamera discovered:");
        println!("  Sensor: {}", devices.sensor_name);
        println!("  Media device: {}", devices.media_device.display());
        println!("  Video device: {}", devices.video_device.display());
        println!("  Resolution: {}x{}", width, height);
        
        // Create capture session with raw FDs
        let capture_session = CaptureSession::new(
            video_device.as_raw_fd(),
            media_device.as_raw_fd(),
            width,
            height,
        );
        
        let camera = Self {
            devices: devices.clone(),
            _video_device: video_device,
            _media_device: media_device,
            capture_session,
            width,
            height,
        };

        // Configure the pipeline
        camera.setup_pipeline()?;
        
        Ok(camera)
    }
    
    /// Configure the media pipeline (sensor -> SENINF -> ISP)
    fn setup_pipeline(&self) -> Result<()> {
        println!("\nConfiguring pipeline...");
        
        let media_dev_str = self.devices.media_device.to_string_lossy();
        let video_dev_str = self.devices.video_device.to_string_lossy();
        
        // 1. Enable sensor link
        println!("  Enabling sensor link...");
        self.run_command(vec![
            "media-ctl".to_string(),
            "-d".to_string(),
            media_dev_str.to_string(),
            "-l".to_string(),
            format!("'{}':0 -> '1a040000.seninf':0 [1]", self.devices.sensor_name),
        ])?;
        
        // 2. Set sensor output format (SGRBG10 10-bit)
        println!("  Setting sensor format...");
        self.run_command(vec![
            "media-ctl".to_string(),
            "-d".to_string(),
            media_dev_str.to_string(),
            "-V".to_string(),
            format!("'{}':0 [fmt:SGRBG10_1X10/{}x{}]", 
                self.devices.sensor_name, self.width, self.height),
        ])?;
        
        // 3. Set SENINF sink pad format
        println!("  Setting SENINF sink format...");
        self.run_command(vec![
            "media-ctl".to_string(),
            "-d".to_string(),
            media_dev_str.to_string(),
            "-V".to_string(),
            format!("'1a040000.seninf':0 [fmt:SGRBG10_1X10/{}x{}]", 
                self.width, self.height),
        ])?;
        
        // 4. Set SENINF source pad format
        println!("  Setting SENINF source format...");
        self.run_command(vec![
            "media-ctl".to_string(),
            "-d".to_string(),
            media_dev_str.to_string(),
            "-V".to_string(),
            format!("'1a040000.seninf':4 [fmt:SGRBG10_1X10/{}x{}]", 
                self.width, self.height),
        ])?;
        
        // 5. Set video device format to MBg8 (8-bit Bayer GRBG)
        println!("  Setting video device format to MBg8...");
        self.run_command(vec![
            "v4l2-ctl".to_string(),
            "-d".to_string(),
            video_dev_str.to_string(),
            "--set-fmt-video".to_string(),
            format!("width={},height={},pixelformat=MBg8", self.width, self.height),
        ])?;
        
        println!("  Pipeline configured successfully");
        
        Ok(())
    }
    
    /// Run an external command (media-ctl, v4l2-ctl, etc)
    fn run_command(&self, args: Vec<String>) -> Result<()> {
        let cmd = args[0].clone();
        let output = Command::new(&cmd)
            .args(&args[1..])
            .output()
            .with_context(|| format!("Failed to run {}", cmd))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            anyhow::bail!(
                "{} failed:\nstdout: {}\nstderr: {}",
                cmd, stdout, stderr
            );
        }
        
        Ok(())
    }
    
    /// Capture a single frame using Media Request API
    pub fn capture_frame(&mut self) -> Result<BayerFrame> {
        self.capture_session.capture_frame()
    }
    
    /// Get frame dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}
