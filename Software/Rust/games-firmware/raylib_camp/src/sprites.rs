//! Direct blitting of raw RGB565 sprite data into the canvas.

use crate::canvas::Canvas;
use crate::color::Color;

impl<'a> Canvas<'a> {
    /// Blits a block of raw RGB565 pixels at `(x, y)`.
    ///
    /// `pixels` is a row-major array of `width * height` colours. Each source
    /// pixel is drawn as an integer-sized `scale` by `scale` block so sprites
    /// can be enlarged without extra asset data. Pixels whose value equals
    /// `transparent` are skipped, letting sprites carry transparent areas.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_sprite(
        &mut self,
        pixels: &[u16],
        x: i32,
        y: i32,
        width: u16,
        height: u16,
        scale: i32,
        transparent: Option<Color>,
    ) {
        let scale = scale.max(1);
        let width = width as usize;
        let height = height as usize;
        let key = transparent.map(Color::raw);
        for source_row in 0..height {
            for source_col in 0..width {
                let colour = pixels[source_row * width + source_col];
                if key == Some(colour) {
                    continue;
                }
                let block_x = x + (source_col as i32) * scale;
                let block_y = y + (source_row as i32) * scale;
                self.rect_filled(block_x, block_y, scale, scale, Color::from_raw(colour));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::canvas::{display, Canvas};
    use crate::color::palette;

    fn single_pixel_sprite() -> Vec<u16> {
        vec![palette::WHITE.raw()]
    }

    #[test]
    fn blit_places_single_pixel_at_offset() {
        let mut storage = vec![0u16; display::PIXEL_COUNT];
        let mut canvas = Canvas::new(&mut storage);
        canvas.draw_sprite(&single_pixel_sprite(), 10, 20, 1, 1, 1, None);
        assert_eq!(storage[20 * display::WIDTH + 10], palette::WHITE.raw());
        assert_eq!(storage[21 * display::WIDTH + 10], 0);
    }

    #[test]
    fn scale_expands_a_single_pixel() {
        let mut storage = vec![0u16; display::PIXEL_COUNT];
        let mut canvas = Canvas::new(&mut storage);
        canvas.draw_sprite(&single_pixel_sprite(), 0, 0, 1, 1, 3, None);
        let painted = storage
            .iter()
            .filter(|&&p| p == palette::WHITE.raw())
            .count();
        assert_eq!(painted, 9);
    }

    #[test]
    fn transparent_key_is_skipped() {
        let mut storage = vec![0u16; display::PIXEL_COUNT];
        let mut canvas = Canvas::new(&mut storage);
        // Start on a non-black background so black can be counted reliably.
        canvas.clear(palette::GREY);
        // One transparent and one solid pixel in a 2x1 sprite.
        let sprite = vec![0x0000u16, palette::RED.raw()];
        canvas.draw_sprite(&sprite, 100, 100, 2, 1, 1, Some(palette::BLACK));
        let red = storage.iter().filter(|&&p| p == palette::RED.raw()).count();
        let black = storage
            .iter()
            .filter(|&&p| p == palette::BLACK.raw())
            .count();
        let grey = storage
            .iter()
            .filter(|&&p| p == palette::GREY.raw())
            .count();
        assert_eq!(red, 1, "solid sprite pixel not drawn once");
        assert_eq!(black, 0, "transparent pixel should not paint background");
        assert_eq!(
            grey,
            display::PIXEL_COUNT - 1,
            "background should stay intact"
        );
    }
}
