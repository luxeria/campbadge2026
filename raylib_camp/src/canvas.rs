//! The draw target: a round 240x240 RGB565 framebuffer.

use crate::color::Color;

/// Fixed geometry of the badge's round display.
pub mod display {
    /// Width of the display in pixels.
    pub const WIDTH: usize = 240;
    /// Height of the display in pixels.
    pub const HEIGHT: usize = 240;
    /// Horizontal centre of the round panel.
    pub const CENTER_X: i32 = 120;
    /// Vertical centre of the round panel.
    pub const CENTER_Y: i32 = 120;
    /// Radius of the round panel in pixels.
    pub const RADIUS: i32 = 120;
    /// Total number of pixels in the framebuffer.
    pub const PIXEL_COUNT: usize = WIDTH * HEIGHT;
}

/// An RGB565 framebuffer that rasterizers draw into.
///
/// The backing storage is owned by the caller (the firmware owns a `static`
/// so the framebuffer lives at a fixed address and can be handed to the
/// display controller). The canvas only borrows that storage and performs
/// in-place pixel writes, which allows the whole engine to stay allocation
/// free and hence `no_std`.
pub struct Canvas<'a> {
    buffer: &'a mut [u16],
}

impl<'a> Canvas<'a> {
    /// Borrows a buffer of exactly [`display::PIXEL_COUNT`] pixels.
    pub fn new(buffer: &'a mut [u16]) -> Self {
        debug_assert_eq!(buffer.len(), display::PIXEL_COUNT);
        Canvas { buffer }
    }

    /// Fills the entire framebuffer with a single colour.
    pub fn clear(&mut self, color: Color) {
        self.buffer.fill(color.raw());
    }

    /// Sets a single pixel, ignoring coordinates outside the display.
    pub fn plot(&mut self, x: i32, y: i32, color: Color) {
        if let Some(index) = Self::index(x, y) {
            self.buffer[index] = color.raw();
        }
    }

    /// Fills a horizontal run of pixels within one scanline.
    ///
    /// The run is silently clipped to the visible area, so callers may pass
    /// out-of-range coordinates. The `x_end` coordinate is exclusive.
    pub(crate) fn fill_row(&mut self, y: i32, x_start: i32, x_end: i32, color: Color) {
        let row = y as usize;
        if row >= display::HEIGHT {
            return;
        }
        let first = x_start.max(0) as usize;
        let last = (x_end.max(0)).min(display::WIDTH as i32) as usize;
        if first >= last {
            return;
        }
        let base = row * display::WIDTH;
        for x in first..last {
            self.buffer[base + x] = color.raw();
        }
    }

    /// Exposes the raw framebuffer so the firmware can transfer it to the
    /// display controller at the end of a frame.
    pub fn as_slice(&self) -> &[u16] {
        self.buffer
    }

    fn index(x: i32, y: i32) -> Option<usize> {
        let px = x as usize;
        let py = y as usize;
        if px < display::WIDTH && py < display::HEIGHT {
            Some(py * display::WIDTH + px)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::display;
    use super::Canvas;
    use crate::color::palette;

    #[test]
    fn clear_fills_every_pixel() {
        let mut storage = vec![0u16; display::PIXEL_COUNT];
        let mut canvas = Canvas::new(&mut storage);
        canvas.clear(palette::RED);
        assert!(canvas.as_slice().iter().all(|&p| p == palette::RED.raw()));
    }

    #[test]
    fn plot_outside_display_is_ignored() {
        let mut storage = vec![0u16; display::PIXEL_COUNT];
        let mut canvas = Canvas::new(&mut storage);
        canvas.plot(-1, 0, palette::WHITE);
        canvas.plot(0, -1, palette::WHITE);
        canvas.plot(display::WIDTH as i32, 0, palette::WHITE);
        canvas.plot(0, display::HEIGHT as i32, palette::WHITE);
        assert!(canvas.as_slice().iter().all(|&p| p == 0));
    }

    #[test]
    fn fill_row_handles_overhang() {
        let mut storage = vec![0u16; display::PIXEL_COUNT];
        let mut canvas = Canvas::new(&mut storage);
        canvas.fill_row(10, -5, display::WIDTH as i32 + 5, palette::BLUE);
        let row_base = 10 * display::WIDTH;
        let row = &canvas.as_slice()[row_base..row_base + display::WIDTH];
        assert!(row.iter().all(|&p| p == palette::BLUE.raw()));
    }
}
