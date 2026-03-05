//! Test pattern generation

/// Generate YUYV color bars test pattern
/// 8 vertical bars: White, Yellow, Cyan, Green, Magenta, Red, Blue, Black
pub struct ColorBars {
    buffer: Vec<u8>,
}

impl ColorBars {
    /// Create a new color bars pattern
    pub fn new(width: u32, height: u32) -> Self {
        let buffer = Self::generate(width, height);
        Self { buffer }
    }

    /// Get the pattern data
    pub fn data(&self) -> &[u8] {
        &self.buffer
    }

    /// Generate the YUYV color bars pattern
    fn generate(width: u32, height: u32) -> Vec<u8> {
        let frame_size = (width * height * 2) as usize;
        let mut buffer = vec![0u8; frame_size];
        
        // YUYV color values for standard color bars
        // Format: [Y0, U, Y1, V] for 2 pixels
        let colors: [(u8, u8, u8, u8); 8] = [
            (235, 128, 235, 128), // White
            (210, 16, 210, 146),  // Yellow
            (170, 166, 170, 16),  // Cyan
            (145, 54, 145, 34),   // Green
            (106, 202, 106, 222), // Magenta
            (81, 90, 81, 240),    // Red
            (41, 240, 41, 110),   // Blue
            (16, 128, 16, 128),   // Black
        ];

        let bar_width = width / 8;
        
        for y in 0..height {
            for x in 0..width {
                let bar_index = (x / bar_width).min(7) as usize;
                let (y0, u, y1, v) = colors[bar_index];
                
                let pixel_offset = ((y * width + x) * 2) as usize;
                
                if x % 2 == 0 {
                    // Y0 U position
                    buffer[pixel_offset] = y0;
                    buffer[pixel_offset + 1] = u;
                } else {
                    // Y1 V position
                    buffer[pixel_offset] = y1;
                    buffer[pixel_offset + 1] = v;
                }
            }
        }
        
        buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_bars_size() {
        let width = 640;
        let height = 480;
        let pattern = ColorBars::new(width, height);
        assert_eq!(pattern.data().len(), (width * height * 2) as usize);
    }
}
