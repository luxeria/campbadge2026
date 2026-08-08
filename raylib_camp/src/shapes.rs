//! Immediate-mode 2D raster shapes drawn into a [`Canvas`].

use crate::canvas::Canvas;
use crate::color::Color;

impl<'a> Canvas<'a> {
    /// Fills an axis-aligned rectangle spanning `width` by `height` pixels
    /// starting at the top-left corner `(x, y)`.
    pub fn rect_filled(&mut self, x: i32, y: i32, width: i32, height: i32, color: Color) {
        let x_end = x.saturating_add(width);
        if width <= 0 || height <= 0 {
            return;
        }
        for row in 0..height {
            self.fill_row(y + row, x, x_end, color);
        }
    }

    /// Draws the outline of an axis-aligned rectangle.
    pub fn rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: Color) {
        if width <= 0 || height <= 0 {
            return;
        }
        let x_end = x.saturating_add(width - 1);
        let y_end = y.saturating_add(height - 1);
        self.line(x, y, x_end, y, color);
        self.line(x, y_end, x_end, y_end, color);
        self.line(x, y, x, y_end, color);
        self.line(x_end, y, x_end, y_end, color);
    }

    /// Draws a line between two points using Bresenham's algorithm.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
        let mut x = x0;
        let mut y = y0;
        let delta_x = (x1 - x0).abs();
        let delta_y = -(y1 - y0).abs();
        let step_x = if x0 < x1 { 1 } else { -1 };
        let step_y = if y0 < y1 { 1 } else { -1 };
        let mut error = delta_x + delta_y;
        loop {
            self.plot(x, y, color);
            if x == x1 && y == y1 {
                break;
            }
            let doubled = 2 * error;
            if doubled >= delta_y {
                error += delta_y;
                x += step_x;
            }
            if doubled <= delta_x {
                error += delta_x;
                y += step_y;
            }
        }
    }

    /// Fills a disc of the given radius around `(center_x, center_y)`.
    pub fn circle_filled(&mut self, center_x: i32, center_y: i32, radius: i32, color: Color) {
        if radius <= 0 {
            return;
        }
        let radius_squared = radius.saturating_mul(radius);
        for offset_y in -radius..=radius {
            let half_width_squared = radius_squared - offset_y * offset_y;
            if half_width_squared < 0 {
                continue;
            }
            let half_width = isqrt(half_width_squared);
            let row_y = center_y + offset_y;
            self.fill_row(
                row_y,
                center_x - half_width,
                center_x + half_width + 1,
                color,
            );
        }
    }

    /// Draws the outline of a circle using the midpoint circle algorithm.
    pub fn circle(&mut self, center_x: i32, center_y: i32, radius: i32, color: Color) {
        if radius <= 0 {
            return;
        }
        let mut x = radius;
        let mut y = 0;
        let mut error = 1 - radius;
        while x >= y {
            self.plot(center_x + x, center_y + y, color);
            self.plot(center_x - x, center_y + y, color);
            self.plot(center_x + x, center_y - y, color);
            self.plot(center_x - x, center_y - y, color);
            self.plot(center_x + y, center_y + x, color);
            self.plot(center_x - y, center_y + x, color);
            self.plot(center_x + y, center_y - x, color);
            self.plot(center_x - y, center_y - x, color);
            y += 1;
            if error < 0 {
                error += 2 * y + 1;
            } else {
                x -= 1;
                error += 2 * (y - x) + 1;
            }
        }
    }

    /// Fills the triangle bounded by the three given vertices.
    pub fn triangle_filled(&mut self, a: (i32, i32), b: (i32, i32), c: (i32, i32), color: Color) {
        let y_min = a.1.min(b.1).min(c.1);
        let y_max = a.1.max(b.1).max(c.1);
        let edges = [(a, b), (b, c), (c, a)];
        for y in y_min..=y_max {
            let mut crossings = [0i32; 2];
            let mut count = 0;
            for &((x0, y0), (x1, y1)) in &edges {
                if ((y0 <= y && y < y1) || (y1 <= y && y < y0)) && count < 2 {
                    let fraction = (y - y0) as f32 / (y1 - y0) as f32;
                    crossings[count] = (x0 as f32 + (x1 - x0) as f32 * fraction) as i32;
                    count += 1;
                }
            }
            if count == 2 {
                let left = crossings[0].min(crossings[1]);
                let right = crossings[0].max(crossings[1]);
                self.fill_row(y, left, right + 1, color);
            }
        }
    }

    /// Draws the outline of a triangle as three line segments.
    pub fn triangle(&mut self, a: (i32, i32), b: (i32, i32), c: (i32, i32), color: Color) {
        self.line(a.0, a.1, b.0, b.1, color);
        self.line(b.0, b.1, c.0, c.1, color);
        self.line(c.0, c.1, a.0, a.1, color);
    }
}

/// Computes the integer square root using Newton's method.
///
/// Used for whole-pixel disc rasterization where a floating point square root
/// is unnecessary. `half_width_squared` is non-negative by construction so
/// the function is only meaningful for `value >= 0`.
fn isqrt(value: i32) -> i32 {
    if value <= 0 {
        return 0;
    }
    let n = value as u32;
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x as i32
}

#[cfg(test)]
mod tests {
    use super::Canvas;
    use crate::canvas::display;
    use crate::color::palette;

    fn count_colored(storage: &[u16], color: u16) -> usize {
        storage.iter().filter(|&&p| p == color).count()
    }

    #[test]
    fn rect_filled_paints_exactly_its_area() {
        let mut storage = vec![0u16; display::PIXEL_COUNT];
        let mut canvas = Canvas::new(&mut storage);
        canvas.rect_filled(10, 10, 20, 30, palette::WHITE);
        assert_eq!(count_colored(&storage, palette::WHITE.raw()), 20 * 30);
    }

    #[test]
    fn rect_filled_clips_at_display_border() {
        let mut storage = vec![0u16; display::PIXEL_COUNT];
        let mut canvas = Canvas::new(&mut storage);
        canvas.rect_filled(-5, -5, 10, 10, palette::WHITE);
        assert_eq!(count_colored(&storage, palette::WHITE.raw()), 5 * 5);
    }

    #[test]
    fn circle_filled_is_symmetric() {
        let mut storage = vec![0u16; display::PIXEL_COUNT];
        let mut canvas = Canvas::new(&mut storage);
        canvas.circle_filled(100, 100, 40, palette::GREEN);
        let painted = count_colored(&storage, palette::GREEN.raw());
        let expected = core::f64::consts::PI * 40.0 * 40.0;
        let tolerance = painted as f64 * 0.15;
        assert!(
            (painted as f64 - expected).abs() < tolerance,
            "filled disc area {painted} deviates too much from {expected}"
        );
    }

    #[test]
    fn triangle_filled_covers_convex_interior() {
        let mut storage = vec![0u16; display::PIXEL_COUNT];
        let mut canvas = Canvas::new(&mut storage);
        // Right triangle with base 100 and height 40, exact area 2000.
        canvas.triangle_filled((50, 50), (150, 50), (50, 90), palette::MAGENTA);
        let painted = count_colored(&storage, palette::MAGENTA.raw());
        assert!(
            (1800..2300).contains(&painted),
            "unexpected painted area {painted}"
        );
    }
}
