mod camera;
mod capture;
mod frame;
mod loopback;
mod media;
mod pattern;

use anyhow::Result;
use clap::Parser;
use std::time::{Duration, Instant};

use loopback::LoopbackDevice;
use pattern::ColorBars;

/// MT8183 Camera Service - Feeds frames to v4l2loopback device
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to v4l2loopback device (e.g., /dev/video10)
    #[arg(short, long, default_value = "/dev/video10")]
    device: String,

    /// Frame width (ignored in camera mode - uses native/2)
    #[arg(long, default_value = "640")]
    width: u32,

    /// Frame height (ignored in camera mode - uses native/2)
    #[arg(long, default_value = "480")]
    height: u32,

    /// Target frames per second
    #[arg(long, default_value = "30")]
    fps: u32,
    
    /// Use real camera instead of test pattern
    #[arg(long)]
    camera: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("MT8183 Camera Service");
    println!("====================");
    println!("Device: {}", args.device);
    println!("Mode: {}", if args.camera { "Real camera" } else { "Test pattern" });
    println!();
    
    // Check if device exists
    if !std::path::Path::new(&args.device).exists() {
        eprintln!("Error: Device {} not found", args.device);
        eprintln!();
        eprintln!("Make sure v4l2loopback module is loaded:");
        eprintln!("  sudo modprobe v4l2loopback video_nr=10 card_label=\"MT8183 Camera\" exclusive_caps=1");
        std::process::exit(1);
    }

    if args.camera {
        run_camera_mode(&args)
    } else {
        run_test_pattern_mode(&args)
    }
}

fn run_test_pattern_mode(args: &Args) -> Result<()> {
    println!("Format: YUYV {}x{} @ {}fps", args.width, args.height, args.fps);
    
    // Open loopback device
    let mut loopback = LoopbackDevice::open(&args.device, args.width, args.height)?;

    // Generate test pattern
    let color_bars = ColorBars::new(loopback.width(), loopback.height());
    
    println!("Starting frame output (press Ctrl+C to stop)...");
    
    // Frame timing
    let frame_duration = Duration::from_secs_f64(1.0 / args.fps as f64);
    let mut frame_count = 0u64;
    let start_time = Instant::now();

    loop {
        let frame_start = Instant::now();

        // Write frame (raw test pattern data)
        match loopback.write_raw(color_bars.data()) {
            Ok(bytes_written) => {
                if bytes_written != loopback.frame_size() {
                    eprintln!(
                        "Warning: Expected to write {} bytes, wrote {}",
                        loopback.frame_size(),
                        bytes_written
                    );
                }
            }
            Err(e) => {
                eprintln!("Error writing frame: {}", e);
                break;
            }
        }

        frame_count += 1;

        // Print stats every 30 frames
        if frame_count % 30 == 0 {
            let elapsed = start_time.elapsed().as_secs_f64();
            let actual_fps = frame_count as f64 / elapsed;
            print!("\rFrames: {} | FPS: {:.2}  ", frame_count, actual_fps);
            std::io::Write::flush(&mut std::io::stdout()).ok();
        }

        // Sleep to maintain target FPS
        let frame_time = frame_start.elapsed();
        if frame_time < frame_duration {
            std::thread::sleep(frame_duration - frame_time);
        }
    }

    println!("\n\nShutdown complete.");
    println!("Total frames output: {}", frame_count);
    
    Ok(())
}

fn run_camera_mode(args: &Args) -> Result<()> {
    // Open camera
    let mut camera = camera::Camera::open()?;
    let (cam_width, cam_height) = camera.dimensions();
    
    // Derive loopback dimensions from camera (native/2 due to demosaic)
    let loopback_width = cam_width / 2;
    let loopback_height = cam_height / 2;
    
    println!("\nOutput dimensions (sensor/2): {}x{}", loopback_width, loopback_height);
    println!("Format: YUYV @ {}fps", args.fps);
    
    // Open loopback device with derived dimensions
    let mut loopback = LoopbackDevice::open(&args.device, loopback_width, loopback_height)?;
    
    println!("Starting camera capture (press Ctrl+C to stop)...\n");
    
    // Frame timing
    let frame_duration = Duration::from_secs_f64(1.0 / args.fps as f64);
    let mut frame_count = 0u64;
    let start_time = Instant::now();

    loop {
        let frame_start = Instant::now();

        // Capture frame from camera
        match camera.capture_frame() {
            Ok(bayer_frame) => {
                // Convert Bayer GRBG to YUYV (half resolution)
                let yuyv_frame = bayer_frame.to_yuyv();
                
                // Write to loopback
                match loopback.write_frame(&yuyv_frame) {
                    Ok(bytes_written) => {
                        if bytes_written != loopback.frame_size() {
                            eprintln!(
                                "Warning: Expected to write {} bytes, wrote {}",
                                loopback.frame_size(),
                                bytes_written
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("Error writing frame: {}", e);
                        break;
                    }
                }
                
                frame_count += 1;

                // Print stats every 10 frames (less frequent due to capture overhead)
                if frame_count % 10 == 0 {
                    let elapsed = start_time.elapsed().as_secs_f64();
                    let actual_fps = frame_count as f64 / elapsed;
                    print!("\rFrames: {} | FPS: {:.2}  ", frame_count, actual_fps);
                    std::io::Write::flush(&mut std::io::stdout()).ok();
                }
            }
            Err(e) => {
                eprintln!("Error capturing frame: {}", e);
                break;
            }
        }

        // Sleep to maintain target FPS
        let frame_time = frame_start.elapsed();
        if frame_time < frame_duration {
            std::thread::sleep(frame_duration - frame_time);
        }
    }

    println!("\n\nShutdown complete.");
    println!("Total frames captured: {}", frame_count);
    
    Ok(())
}
