//! Media device discovery and handling

use anyhow::{Context, Result};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Discovered camera device paths
#[derive(Debug, Clone)]
pub struct CameraDevices {
    /// Main video capture device (e.g., /dev/video3)
    pub video_device: PathBuf,
    /// Media controller device (e.g., /dev/media0)
    pub media_device: PathBuf,
    /// Sensor name for debugging
    pub sensor_name: String,
}

/// Find the MT8183 ISP media device by parsing v4l2-ctl output
fn find_isp_media_device() -> Result<PathBuf> {
    let output = Command::new("v4l2-ctl")
        .arg("--list-devices")
        .output()
        .context("Failed to run v4l2-ctl")?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut in_mtk_cam = false;
    
    for line in stdout.lines() {
        if line.contains("mtk-cam-p1") {
            in_mtk_cam = true;
            continue;
        }
        
        if in_mtk_cam {
            let trimmed = line.trim();
            if trimmed.starts_with("/dev/media") {
                return Ok(PathBuf::from(trimmed));
            }
            // New section started
            if !trimmed.is_empty() && !trimmed.starts_with("/dev/") {
                in_mtk_cam = false;
            }
        }
    }
    
    anyhow::bail!("Could not find mtk-cam-p1 media device. Is the camera module loaded?")
}

/// Find the main stream video device from media topology
fn find_main_stream_device(media_device: &Path) -> Result<PathBuf> {
    let output = Command::new("media-ctl")
        .args(&["-d", media_device.to_str().unwrap(), "-p"])
        .output()
        .context("Failed to run media-ctl")?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut found_main_stream = false;
    
    for line in stdout.lines() {
        if line.contains("entity") && line.contains("main stream") {
            found_main_stream = true;
            continue;
        }
        
        if found_main_stream && line.contains("device node name") {
            // Extract /dev/videoN pattern from the line
            // Pattern: /dev/video followed by one or more digits
            if let Some(start) = line.find("/dev/video") {
                let rest = &line[start..];
                // Count digits after "/dev/video" (10 characters)
                let digit_count = rest[10..]
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .count();
                
                let video_path = &rest[..10 + digit_count]; // "/dev/video" + digits
                return Ok(PathBuf::from(video_path));
            }
            found_main_stream = false;
        }
        
        if found_main_stream && line.trim().starts_with("- entity") {
            found_main_stream = false;
        }
    }
    
    anyhow::bail!("Could not find main stream video device in media topology")
}

/// Find the active sensor (ov8856 or ov02a10)
fn find_sensor_name(media_device: &Path) -> Result<String> {
    let output = Command::new("media-ctl")
        .args(&["-d", media_device.to_str().unwrap(), "-p"])
        .output()
        .context("Failed to run media-ctl")?;
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    for line in stdout.lines() {
        if line.contains("entity") && (line.contains("ov8856") || line.contains("ov02a10")) {
            // Format: "- entity NN: ov8856 2-0010 (..."
            // Extract the part between colon and opening parenthesis
            if let Some(colon_pos) = line.find(':') {
                if let Some(paren_pos) = line.find('(') {
                    if colon_pos < paren_pos {
                        let entity_name = line[colon_pos + 1..paren_pos]
                            .trim()
                            .to_string();
                        if !entity_name.is_empty() {
                            return Ok(entity_name);
                        }
                    }
                }
            }
        }
    }
    
    anyhow::bail!("Could not find camera sensor (ov8856 or ov02a10)")
}

/// Discover all camera devices automatically
pub fn discover() -> Result<CameraDevices> {
    println!("Discovering camera devices...");
    
    let media_device = find_isp_media_device()
        .context("Failed to find ISP media device")?;
    println!("  Media device: {}", media_device.display());
    
    let video_device = find_main_stream_device(&media_device)
        .context("Failed to find main stream video device")?;
    println!("  Video device: {}", video_device.display());
    
    let sensor_name = find_sensor_name(&media_device)
        .context("Failed to find camera sensor")?;
    println!("  Sensor: {}", sensor_name);
    
    Ok(CameraDevices {
        video_device,
        media_device,
        sensor_name,
    })
}

/// Open a media device file
pub fn open_media_device(path: &Path) -> Result<File> {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .with_context(|| format!("Failed to open media device: {}", path.display()))
}
