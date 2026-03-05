//! Frame types for camera capture and output
//!
//! Provides strongly-typed wrappers for raw Bayer and YUYV frame data,
//! including conversion between formats.

/// Bayer color filter array pattern
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BayerPattern {
    /// Green-Red / Blue-Green (MT8183 OV8856 uses this)
    GRBG,
    /// Red-Green / Green-Blue
    RGGB,
    /// Blue-Green / Green-Red
    BGGR,
    /// Green-Blue / Red-Green
    GBRG,
}

/// A raw Bayer frame captured from the camera sensor
#[derive(Debug, Clone)]
pub struct BayerFrame {
    data: Vec<u8>,
    width: u32,
    height: u32,
    pattern: BayerPattern,
}

impl BayerFrame {
    /// Create a new BayerFrame from raw data
    ///
    /// # Panics
    /// Panics if data.len() != width * height
    pub fn new(data: Vec<u8>, width: u32, height: u32, pattern: BayerPattern) -> Self {
        let expected = (width * height) as usize;
        assert_eq!(
            data.len(),
            expected,
            "BayerFrame data size mismatch: expected {}, got {}",
            expected,
            data.len()
        );
        Self {
            data,
            width,
            height,
            pattern,
        }
    }

    /// Raw frame data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Frame width in pixels
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Frame height in pixels
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Bayer pattern used by this frame
    pub fn pattern(&self) -> BayerPattern {
        self.pattern
    }

    /// Convert to YUYV format for v4l2loopback output
    ///
    /// Performs demosaicing using simple 2x2 block averaging.
    /// Output is half resolution (width/2 x height/2).
    pub fn to_yuyv(&self) -> YuyvFrame {
        match self.pattern {
            BayerPattern::GRBG => self.grbg_to_yuyv(),
            _ => unimplemented!("Only GRBG pattern is currently supported"),
        }
    }

    /// Demosaic GRBG Bayer to YUYV
    ///
    /// Bayer pattern (GRBG):
    ///   Row 0: G R G R ...  (even rows: G at even cols, R at odd cols)
    ///   Row 1: B G B G ...  (odd rows: B at even cols, G at odd cols)
    ///
    /// Uses simple 2x2 block averaging for speed.
    fn grbg_to_yuyv(&self) -> YuyvFrame {
        let out_width = self.width / 2;
        let out_height = self.height / 2;
        // YUYV: 2 bytes per pixel (Y for each pixel, U/V shared between pairs)
        let mut yuyv = vec![0u8; (out_width * out_height * 2) as usize];

        let width = self.width as usize;

        for row in 0..out_height as usize {
            for col in 0..out_width as usize {
                // Extract 2x2 Bayer block
                let bayer_y = row * 2;
                let bayer_x = col * 2;

                let idx00 = bayer_y * width + bayer_x;
                let idx01 = bayer_y * width + bayer_x + 1;
                let idx10 = (bayer_y + 1) * width + bayer_x;
                let idx11 = (bayer_y + 1) * width + bayer_x + 1;

                // GRBG pattern:
                //   [G][R]  <- even row
                //   [B][G]  <- odd row
                let g1 = self.data[idx00] as u16; // Green at (even row, even col)
                let r = self.data[idx01] as u16; // Red at (even row, odd col)
                let b = self.data[idx10] as u16; // Blue at (odd row, even col)
                let g2 = self.data[idx11] as u16; // Green at (odd row, odd col)

                // Average the two greens
                let g = ((g1 + g2) / 2) as i32;
                let r = r as i32;
                let b = b as i32;

                // Convert RGB to YUV using ITU-R BT.601
                // Y  = 0.299*R + 0.587*G + 0.114*B
                // U  = -0.169*R - 0.331*G + 0.500*B + 128
                // V  = 0.500*R - 0.419*G - 0.081*B + 128
                let y_val = (77 * r + 150 * g + 29 * b) >> 8; // Scaled by 256
                let u_val = ((-43 * r - 85 * g + 128 * b) >> 8) + 128;
                let v_val = ((128 * r - 107 * g - 21 * b) >> 8) + 128;

                let y = y_val.clamp(0, 255) as u8;
                let u = u_val.clamp(0, 255) as u8;
                let v = v_val.clamp(0, 255) as u8;

                // YUYV packing: Y0 U Y1 V for each pair of horizontal pixels
                // Since we're at half resolution, we write Y U Y V for this single output pixel
                let yuyv_idx = (row * out_width as usize + col) * 2;
                if col % 2 == 0 {
                    // Even column: Y U
                    yuyv[yuyv_idx] = y;
                    yuyv[yuyv_idx + 1] = u;
                } else {
                    // Odd column: Y V
                    yuyv[yuyv_idx] = y;
                    yuyv[yuyv_idx + 1] = v;
                }
            }
        }

        YuyvFrame::new(yuyv, out_width, out_height)
    }

    /// Apply brightness boost (multiply pixel values, clamped to 255)
    pub fn boost_brightness(&mut self, factor: u8) {
        if factor <= 1 {
            return;
        }
        for pixel in self.data.iter_mut() {
            let boosted = (*pixel as u16) * (factor as u16);
            *pixel = boosted.min(255) as u8;
        }
    }
}

/// A YUYV (YUV 4:2:2) frame for output to v4l2loopback
#[derive(Debug, Clone)]
pub struct YuyvFrame {
    data: Vec<u8>,
    width: u32,
    height: u32,
}

impl YuyvFrame {
    /// Create a new YuyvFrame from raw YUYV data
    ///
    /// YUYV is 2 bytes per pixel (Y for each, U/V shared between horizontal pairs)
    ///
    /// # Panics
    /// Panics if data.len() != width * height * 2
    pub fn new(data: Vec<u8>, width: u32, height: u32) -> Self {
        let expected = (width * height * 2) as usize;
        assert_eq!(
            data.len(),
            expected,
            "YuyvFrame data size mismatch: expected {}, got {}",
            expected,
            data.len()
        );
        Self {
            data,
            width,
            height,
        }
    }

    /// Raw YUYV data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Frame width in pixels
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Frame height in pixels
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Frame size in bytes
    pub fn byte_len(&self) -> usize {
        self.data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bayer_frame_creation() {
        let width = 640;
        let height = 480;
        let data = vec![0u8; width * height];
        let frame = BayerFrame::new(data, width as u32, height as u32, BayerPattern::GRBG);

        assert_eq!(frame.width(), width as u32);
        assert_eq!(frame.height(), height as u32);
        assert_eq!(frame.pattern(), BayerPattern::GRBG);
    }

    #[test]
    fn test_bayer_to_yuyv_dimensions() {
        let width = 640u32;
        let height = 480u32;
        let data = vec![128u8; (width * height) as usize];
        let bayer = BayerFrame::new(data, width, height, BayerPattern::GRBG);

        let yuyv = bayer.to_yuyv();

        // Output should be half resolution
        assert_eq!(yuyv.width(), width / 2);
        assert_eq!(yuyv.height(), height / 2);
        // YUYV is 2 bytes per pixel
        assert_eq!(yuyv.byte_len(), (width / 2 * height / 2 * 2) as usize);
    }

    #[test]
    fn test_brightness_boost() {
        let mut frame = BayerFrame::new(vec![100u8; 4], 2, 2, BayerPattern::GRBG);
        frame.boost_brightness(2);
        assert_eq!(frame.data()[0], 200);

        let mut frame = BayerFrame::new(vec![200u8; 4], 2, 2, BayerPattern::GRBG);
        frame.boost_brightness(2);
        assert_eq!(frame.data()[0], 255); // Clamped
    }
}
