//! V4L2 frame capture using Media Request API
//!
//! The MT8183 camera ISP requires the Media Request API because it uses
//! an SCP (System Control Processor) for frame processing. Simple V4L2
//! streaming won't work - frames must be queued with requests.
//!
//! Flow:
//! 1. Allocate kernel buffers (VIDIOC_REQBUFS)
//! 2. Query and mmap buffers (VIDIOC_QUERYBUF)
//! 3. For each frame:
//!    a. Allocate request (MEDIA_IOC_REQUEST_ALLOC)
//!    b. Queue buffer with request (VIDIOC_QBUF + V4L2_BUF_FLAG_REQUEST_FD)
//!    c. Submit request (MEDIA_REQUEST_IOC_QUEUE)
//!    d. Start streaming (VIDIOC_STREAMON)
//!    e. Wait for completion (select on request_fd)
//!    f. Dequeue buffer (VIDIOC_DQBUF)
//!    g. Stop streaming (VIDIOC_STREAMOFF)
//!    h. Close request fd

use anyhow::{bail, Result};
use std::os::unix::io::RawFd;

use crate::frame::{BayerFrame, BayerPattern};

// =============================================================================
// ioctl code generation (matching Linux kernel _IOC macro)
// =============================================================================

const IOC_NONE: u32 = 0;
const IOC_WRITE: u32 = 1;
const IOC_READ: u32 = 2;

const fn ioc(dir: u32, type_: u8, nr: u8, size: usize) -> libc::c_ulong {
    ((dir as libc::c_ulong) << 30)
        | ((size as libc::c_ulong) << 16)
        | ((type_ as libc::c_ulong) << 8)
        | (nr as libc::c_ulong)
}

// V4L2 ioctls (type 'V' = 0x56)
const VIDIOC_REQBUFS: libc::c_ulong = ioc(IOC_READ | IOC_WRITE, b'V', 8, 20); // sizeof(v4l2_requestbuffers)
const VIDIOC_QUERYBUF: libc::c_ulong = ioc(IOC_READ | IOC_WRITE, b'V', 9, 88); // sizeof(v4l2_buffer) on 64-bit
const VIDIOC_QBUF: libc::c_ulong = ioc(IOC_READ | IOC_WRITE, b'V', 15, 88);
const VIDIOC_DQBUF: libc::c_ulong = ioc(IOC_READ | IOC_WRITE, b'V', 17, 88);
const VIDIOC_STREAMON: libc::c_ulong = ioc(IOC_WRITE, b'V', 18, 4);
const VIDIOC_STREAMOFF: libc::c_ulong = ioc(IOC_WRITE, b'V', 19, 4);

// Media ioctls (type '|' = 0x7C)
const MEDIA_IOC_REQUEST_ALLOC: libc::c_ulong = ioc(IOC_READ, b'|', 5, 4);

// Request ioctls - also use '|' magic (not '#' as I mistakenly thought)
const MEDIA_REQUEST_IOC_QUEUE: libc::c_ulong = ioc(IOC_NONE, b'|', 0x80, 0);
const MEDIA_REQUEST_IOC_REINIT: libc::c_ulong = ioc(IOC_NONE, b'|', 0x81, 0);

// V4L2 constants
const V4L2_BUF_TYPE_VIDEO_CAPTURE_MPLANE: u32 = 9;
const V4L2_MEMORY_MMAP: u32 = 1;
const V4L2_BUF_FLAG_REQUEST_FD: u32 = 0x00800000;
const V4L2_FIELD_NONE: u32 = 1;

// =============================================================================
// V4L2 structures (MPLANE variants, matching kernel definitions)
// =============================================================================

/// Union for plane memory offset/userptr/fd
#[repr(C)]
#[derive(Copy, Clone)]
pub union V4l2PlaneMemUnion {
    pub mem_offset: u32,
    pub userptr: libc::c_ulong,
    pub fd: i32,
}

/// V4L2 plane info for multiplanar buffers
#[repr(C)]
#[derive(Copy, Clone)]
pub struct V4l2Plane {
    pub bytesused: u32,
    pub length: u32,
    pub m: V4l2PlaneMemUnion,
    pub data_offset: u32,
    pub reserved: [u32; 11],
}

impl Default for V4l2Plane {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

/// V4L2 request buffers structure
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct V4l2Requestbuffers {
    pub count: u32,
    pub type_: u32,
    pub memory: u32,
    pub capabilities: u32,
    pub flags: u8,
    pub reserved: [u8; 3],
}

impl Default for V4l2Requestbuffers {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

/// Union for buffer memory (single-plane offset or multiplane pointer)
#[repr(C)]
#[derive(Copy, Clone)]
pub union V4l2BufferMemUnion {
    pub offset: u32,
    pub userptr: libc::c_ulong,
    pub planes: *mut V4l2Plane,
    pub fd: i32,
}

/// V4L2 timecode structure
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct V4l2Timecode {
    pub type_: u32,
    pub flags: u32,
    pub frames: u8,
    pub seconds: u8,
    pub minutes: u8,
    pub hours: u8,
    pub userbits: [u8; 4],
}

/// V4L2 buffer structure (works for both single and multiplanar)
#[repr(C)]
#[derive(Copy, Clone)]
pub struct V4l2Buffer {
    pub index: u32,
    pub type_: u32,
    pub bytesused: u32,
    pub flags: u32,
    pub field: u32,
    pub timestamp: libc::timeval,
    pub timecode: V4l2Timecode,
    pub sequence: u32,
    pub memory: u32,
    pub m: V4l2BufferMemUnion,
    pub length: u32,
    pub reserved2: u32,
    pub request_fd: u32,
}

impl Default for V4l2Buffer {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

// =============================================================================
// Mapped buffer management
// =============================================================================

/// A kernel buffer mapped into userspace
pub struct MappedBuffer {
    ptr: *mut u8,
    length: usize,
}

impl MappedBuffer {
    /// Map a V4L2 buffer into userspace
    fn map(video_fd: RawFd, offset: u32, length: usize) -> Result<Self> {
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                length,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                video_fd,
                offset as libc::off_t,
            )
        };

        if ptr == libc::MAP_FAILED {
            bail!("mmap failed: {}", std::io::Error::last_os_error());
        }

        Ok(Self {
            ptr: ptr as *mut u8,
            length,
        })
    }

    /// Get buffer data as slice
    fn as_slice(&self, bytesused: usize) -> &[u8] {
        let len = if bytesused > 0 && bytesused <= self.length {
            bytesused
        } else {
            self.length
        };
        unsafe { std::slice::from_raw_parts(self.ptr, len) }
    }
}

impl Drop for MappedBuffer {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.ptr as *mut libc::c_void, self.length);
        }
    }
}

// SAFETY: MappedBuffer contains a raw pointer to mmapped memory which is
// effectively a shared reference to kernel-managed memory. It's safe to
// Send because the kernel ensures proper synchronization.
unsafe impl Send for MappedBuffer {}

// =============================================================================
// Capture session
// =============================================================================

/// Manages V4L2 buffer allocation and capture state
pub struct CaptureSession {
    video_fd: RawFd,
    media_fd: RawFd,
    buffers: Vec<MappedBuffer>,
    width: u32,
    height: u32,
    initialized: bool,
    streaming: bool,
    next_buffer: usize,
}

impl CaptureSession {
    /// Create a new capture session
    pub fn new(video_fd: RawFd, media_fd: RawFd, width: u32, height: u32) -> Self {
        Self {
            video_fd,
            media_fd,
            buffers: Vec::new(),
            width,
            height,
            initialized: false,
            streaming: false,
            next_buffer: 0,
        }
    }

    /// Initialize buffers (call once before capturing)
    pub fn init(&mut self, num_buffers: u32) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        // Request buffers from kernel
        let mut req = V4l2Requestbuffers {
            count: num_buffers,
            type_: V4L2_BUF_TYPE_VIDEO_CAPTURE_MPLANE,
            memory: V4L2_MEMORY_MMAP,
            ..Default::default()
        };

        let ret = unsafe { libc::ioctl(self.video_fd, VIDIOC_REQBUFS, &mut req) };
        if ret < 0 {
            bail!("VIDIOC_REQBUFS failed: {}", std::io::Error::last_os_error());
        }

        if req.count < 1 {
            bail!("Insufficient buffer memory (got {} buffers)", req.count);
        }

        // Query and map each buffer
        for i in 0..req.count {
            let mut plane = V4l2Plane::default();
            let mut buf = V4l2Buffer {
                index: i,
                type_: V4L2_BUF_TYPE_VIDEO_CAPTURE_MPLANE,
                memory: V4L2_MEMORY_MMAP,
                length: 1, // Number of planes
                m: V4l2BufferMemUnion {
                    planes: &mut plane,
                },
                ..Default::default()
            };

            let ret = unsafe { libc::ioctl(self.video_fd, VIDIOC_QUERYBUF, &mut buf) };
            if ret < 0 {
                bail!(
                    "VIDIOC_QUERYBUF failed for buffer {}: {}",
                    i,
                    std::io::Error::last_os_error()
                );
            }

            let offset = unsafe { plane.m.mem_offset };
            let length = plane.length as usize;

            let mapped = MappedBuffer::map(self.video_fd, offset, length)?;
            self.buffers.push(mapped);
        }

        self.initialized = true;
        Ok(())
    }

    /// Capture a single frame using Media Request API
    /// Keeps streaming active between calls for better performance.
    pub fn capture_frame(&mut self) -> Result<BayerFrame> {
        if !self.initialized {
            self.init(4)?;
        }

        // Start streaming on first capture (keep it running)
        if !self.streaming {
            let mut buf_type = V4L2_BUF_TYPE_VIDEO_CAPTURE_MPLANE;
            let ret = unsafe { libc::ioctl(self.video_fd, VIDIOC_STREAMON, &mut buf_type) };
            if ret < 0 {
                bail!("VIDIOC_STREAMON failed: {}", std::io::Error::last_os_error());
            }
            self.streaming = true;
        }

        // Rotate through buffers
        let buffer_index = self.next_buffer as u32;
        self.next_buffer = (self.next_buffer + 1) % self.buffers.len();

        // Allocate a media request
        let mut request_fd: i32 = -1;
        let ret = unsafe { libc::ioctl(self.media_fd, MEDIA_IOC_REQUEST_ALLOC, &mut request_fd) };
        if ret < 0 {
            bail!(
                "MEDIA_IOC_REQUEST_ALLOC failed: {}",
                std::io::Error::last_os_error()
            );
        }

        let mut plane = V4l2Plane::default();

        // Queue buffer with request
        let mut qbuf = V4l2Buffer {
            index: buffer_index,
            type_: V4L2_BUF_TYPE_VIDEO_CAPTURE_MPLANE,
            memory: V4L2_MEMORY_MMAP,
            flags: V4L2_BUF_FLAG_REQUEST_FD,
            field: V4L2_FIELD_NONE,
            length: 1,
            m: V4l2BufferMemUnion {
                planes: &mut plane,
            },
            request_fd: request_fd as u32,
            ..Default::default()
        };

        let ret = unsafe { libc::ioctl(self.video_fd, VIDIOC_QBUF, &mut qbuf) };
        if ret < 0 {
            unsafe { libc::close(request_fd) };
            bail!("VIDIOC_QBUF failed: {}", std::io::Error::last_os_error());
        }

        // Submit the request
        let ret = unsafe { libc::ioctl(request_fd, MEDIA_REQUEST_IOC_QUEUE, std::ptr::null::<()>()) };
        if ret < 0 {
            unsafe { libc::close(request_fd) };
            bail!(
                "MEDIA_REQUEST_IOC_QUEUE failed: {}",
                std::io::Error::last_os_error()
            );
        }

        // Wait for request completion (select on exception fd set)
        let mut except_fds: libc::fd_set = unsafe { std::mem::zeroed() };
        unsafe {
            libc::FD_ZERO(&mut except_fds);
            libc::FD_SET(request_fd, &mut except_fds);
        }

        let mut timeout = libc::timeval {
            tv_sec: 5,
            tv_usec: 0,
        };

        let select_ret = unsafe {
            libc::select(
                request_fd + 1,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut except_fds,
                &mut timeout,
            )
        };

        if select_ret <= 0 {
            unsafe { libc::close(request_fd) };
            if select_ret == 0 {
                bail!("Timeout waiting for frame (5s)");
            } else {
                bail!("select() failed: {}", std::io::Error::last_os_error());
            }
        }

        // Dequeue the buffer
        let mut dq_plane = V4l2Plane::default();
        let mut dqbuf = V4l2Buffer {
            type_: V4L2_BUF_TYPE_VIDEO_CAPTURE_MPLANE,
            memory: V4L2_MEMORY_MMAP,
            length: 1,
            m: V4l2BufferMemUnion {
                planes: &mut dq_plane,
            },
            ..Default::default()
        };

        let ret = unsafe { libc::ioctl(self.video_fd, VIDIOC_DQBUF, &mut dqbuf) };
        if ret < 0 {
            unsafe { libc::close(request_fd) };
            bail!("VIDIOC_DQBUF failed: {}", std::io::Error::last_os_error());
        }

        // Copy frame data
        let bytesused = dq_plane.bytesused as usize;
        let frame_data = self.buffers[dqbuf.index as usize]
            .as_slice(bytesused)
            .to_vec();

        // Cleanup request
        unsafe {
            let _ = libc::ioctl(request_fd, MEDIA_REQUEST_IOC_REINIT, std::ptr::null::<()>());
            libc::close(request_fd);
        }

        // Create typed frame
        Ok(BayerFrame::new(
            frame_data,
            self.width,
            self.height,
            BayerPattern::GRBG,
        ))
    }
}

impl Drop for CaptureSession {
    fn drop(&mut self) {
        // Stop streaming first
        if self.streaming {
            let mut buf_type = V4L2_BUF_TYPE_VIDEO_CAPTURE_MPLANE;
            unsafe {
                let _ = libc::ioctl(self.video_fd, VIDIOC_STREAMOFF, &mut buf_type);
            }
        }
        
        // Buffers are automatically unmapped when dropped
        // Request 0 buffers to free kernel allocations
        if self.initialized {
            let mut req = V4l2Requestbuffers {
                count: 0,
                type_: V4L2_BUF_TYPE_VIDEO_CAPTURE_MPLANE,
                memory: V4L2_MEMORY_MMAP,
                ..Default::default()
            };
            unsafe {
                let _ = libc::ioctl(self.video_fd, VIDIOC_REQBUFS, &mut req);
            }
        }
    }
}
