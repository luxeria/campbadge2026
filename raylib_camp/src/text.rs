//! Bitmap text rendering using an embedded font copied from raylib.
//!
//! raylib ships its default font as a 128x128 bit-packed atlas generated from a
//! 512-word constant plus a 224-entry width table (see `rtext.c`). The data and
//! the glyph-packing layout are reproduced here so the badge renders the same
//! crisp small type without loading any external asset. The original is
//! distributed under the zlib license; see `raylib/` in the repository root.

use crate::canvas::Canvas;
use crate::color::Color;

/// Number of glyphs covered by the embedded font (characters 32..255).
const GLYPH_COUNT: usize = 224;
/// Code point of the first glyph in the font.
const FIRST_CHAR: u8 = 32;
/// Edge length of the square font atlas in pixels.
const ATLAS_SIZE: usize = 128;
/// Pixel height of every glyph.
const GLYPH_HEIGHT: usize = 10;
/// One-pixel gap placed between glyphs and lines in the atlas.
const GLYPH_DIVISOR: usize = 1;
/// The divisor doubled, used when a glyph wraps to a new atlas line.
const DOUBLE_DIVISOR: usize = 2 * GLYPH_DIVISOR;

/// Bit-packed font atlas, reproduced from raylib's `defaultFontData`.
const FONT_ATLAS: [u32; 512] = [
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00200020, 0x0001b000, 0x00000000, 0x00000000,
    0x8ef92520, 0x00020a00, 0x7dbe8000, 0x1f7df45f, 0x4a2bf2a0, 0x0852091e, 0x41224000, 0x10041450,
    0x2e292020, 0x08220812, 0x41222000, 0x10041450, 0x10f92020, 0x3efa084c, 0x7d22103c, 0x107df7de,
    0xe8a12020, 0x08220832, 0x05220800, 0x10450410, 0xa4a3f000, 0x08520832, 0x05220400, 0x10450410,
    0xe2f92020, 0x0002085e, 0x7d3e0281, 0x107df41f, 0x00200000, 0x8001b000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0xc0000fbe, 0xfbf7e00f, 0x5fbf7e7d, 0x0050bee8,
    0x440808a2, 0x0a142fe8, 0x50810285, 0x0050a048, 0x49e428a2, 0x0a142828, 0x40810284, 0x0048a048,
    0x10020fbe, 0x09f7ebaf, 0xd89f3e84, 0x0047a04f, 0x09e48822, 0x0a142aa1, 0x50810284, 0x0048a048,
    0x04082822, 0x0a142fa0, 0x50810285, 0x0050a248, 0x00008fbe, 0xfbf42021, 0x5f817e7d, 0x07d09ce8,
    0x00008000, 0x00000fe0, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x000c0180,
    0xdfbf4282, 0x0bfbf7ef, 0x42850505, 0x004804bf, 0x50a142c6, 0x08401428, 0x42852505, 0x00a808a0,
    0x50a146aa, 0x08401428, 0x42852505, 0x00081090, 0x5fa14a92, 0x0843f7e8, 0x7e792505, 0x00082088,
    0x40a15282, 0x08420128, 0x40852489, 0x00084084, 0x40a16282, 0x0842022a, 0x40852451, 0x00088082,
    0xc0bf4282, 0xf843f42f, 0x7e85fc21, 0x3e0900bf, 0x00000000, 0x00000004, 0x00000000, 0x000c0180,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x04000402, 0x41482000, 0x00000000, 0x00000800,
    0x04000404, 0x4100203c, 0x00000000, 0x00000800, 0xf7df7df0, 0x514bef85, 0xbefbefbe, 0x04513bef,
    0x14414500, 0x494a2885, 0xa28a28aa, 0x04510820, 0xf44145f0, 0x474a289d, 0xa28a28aa, 0x04510be0,
    0x14414510, 0x494a2884, 0xa28a28aa, 0x02910a00, 0xf7df7df0, 0xd14a2f85, 0xbefbe8aa, 0x011f7be0,
    0x00000000, 0x00400804, 0x20080000, 0x00000000, 0x00000000, 0x00600f84, 0x20080000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0xac000000, 0x00000f01, 0x00000000, 0x00000000,
    0x24000000, 0x00000f01, 0x00000000, 0x06000000, 0x24000000, 0x00000f01, 0x00000000, 0x09108000,
    0x24fa28a2, 0x00000f01, 0x00000000, 0x013e0000, 0x2242252a, 0x00000f52, 0x00000000, 0x038a8000,
    0x2422222a, 0x00000f29, 0x00000000, 0x010a8000, 0x2412252a, 0x00000f01, 0x00000000, 0x010a8000,
    0x24fbe8be, 0x00000f01, 0x00000000, 0x0ebe8000, 0xac020000, 0x00000f01, 0x00000000, 0x00048000,
    0x0003e000, 0x00000f00, 0x00000000, 0x00008000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000038, 0x8443b80e, 0x00203a03, 0x02bea080, 0xf0000020, 0xc452208a, 0x04202b02,
    0xf8029122, 0x07f0003b, 0xe44b388e, 0x02203a02, 0x081e8a1c, 0x0411e92a, 0xf4420be0, 0x01248202,
    0xe8140414, 0x05d104ba, 0xe7c3b880, 0x00893a0a, 0x283c0e1c, 0x04500902, 0xc4400080, 0x00448002,
    0xe8208422, 0x04500002, 0x80400000, 0x05200002, 0x083e8e00, 0x04100002, 0x804003e0, 0x07000042,
    0xf8008400, 0x07f00003, 0x80400000, 0x04000022, 0x00000000, 0x00000000, 0x80400000, 0x04000002,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00800702, 0x1848a0c2, 0x84010000, 0x02920921,
    0x01042642, 0x00005121, 0x42023f7f, 0x00291002, 0xefc01422, 0x7efdfbf7, 0xefdfa109, 0x03bbbbf7,
    0x28440f12, 0x42850a14, 0x20408109, 0x01111010, 0x28440408, 0x42850a14, 0x2040817f, 0x01111010,
    0xefc78204, 0x7efdfbf7, 0xe7cf8109, 0x011111f3, 0x2850a932, 0x42850a14, 0x2040a109, 0x01111010,
    0x2850b840, 0x42850a14, 0xefdfbf79, 0x03bbbbf7, 0x001fa020, 0x00000000, 0x00001000, 0x00000000,
    0x00002070, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x08022800, 0x00012283, 0x02430802, 0x01010001, 0x8404147c, 0x20000144, 0x80048404, 0x00823f08,
    0xdfbf4284, 0x7e03f7ef, 0x142850a1, 0x0000210a, 0x50a14684, 0x528a1428, 0x142850a1, 0x03efa17a,
    0x50a14a9e, 0x52521428, 0x142850a1, 0x02081f4a, 0x50a15284, 0x4a221428, 0xf42850a1, 0x03efa14b,
    0x50a16284, 0x4a521428, 0x042850a1, 0x0228a17a, 0xdfbf427c, 0x7e8bf7ef, 0xf7efdfbf, 0x03efbd0b,
    0x00000000, 0x04000000, 0x00000000, 0x00000008, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00200508, 0x00840400, 0x11458122, 0x00014210,
    0x00514294, 0x51420800, 0x20a22a94, 0x0050a508, 0x00200000, 0x00000000, 0x00050000, 0x08000000,
    0xfefbefbe, 0xfbefbefb, 0xfbeb9114, 0x00fbefbe, 0x20820820, 0x8a28a20a, 0x8a289114, 0x3e8a28a2,
    0xfefbefbe, 0xfbefbe0b, 0x8a289114, 0x008a28a2, 0x228a28a2, 0x08208208, 0x8a289114, 0x088a28a2,
    0xfefbefbe, 0xfbefbefb, 0xfa2f9114, 0x00fbefbe, 0x00000000, 0x00000040, 0x00000000, 0x00000000,
    0x00000000, 0x00000020, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00210100, 0x00000004, 0x00000000, 0x00000000, 0x14508200, 0x00001402, 0x00000000, 0x00000000,
    0x00000010, 0x00000020, 0x00000000, 0x00000000, 0xa28a28be, 0x00002228, 0x00000000, 0x00000000,
    0xa28a28aa, 0x000022e8, 0x00000000, 0x00000000, 0xa28a28aa, 0x000022a8, 0x00000000, 0x00000000,
    0xa28a28aa, 0x000022e8, 0x00000000, 0x00000000, 0xbefbefbe, 0x00003e2f, 0x00000000, 0x00000000,
    0x00000004, 0x00002028, 0x00000000, 0x00000000, 0x80000000, 0x00003e0f, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000,
];

/// Per-glyph advance width in pixels, reproduced from raylib's `charsWidth`.
const CHAR_WIDTHS: [u8; GLYPH_COUNT] = [
    3, 1, 4, 6, 5, 7, 6, 2, 3, 3, 5, 5, 2, 4, 1, 7, 5, 2, 5, 5, 5, 5, 5, 5, 5, 5, 1, 1, 3, 4, 3, 6,
    7, 6, 6, 6, 6, 6, 6, 6, 6, 3, 5, 6, 5, 7, 6, 6, 6, 6, 6, 6, 7, 6, 7, 7, 6, 6, 6, 2, 7, 2, 3, 5,
    2, 5, 5, 5, 5, 5, 4, 5, 5, 1, 2, 5, 2, 5, 5, 5, 5, 5, 5, 5, 4, 5, 5, 5, 5, 5, 5, 3, 1, 3, 4, 4,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    1, 1, 5, 5, 5, 7, 1, 5, 3, 7, 3, 5, 4, 1, 7, 4, 3, 5, 3, 3, 2, 5, 6, 1, 2, 2, 3, 5, 6, 6, 6, 6,
    6, 6, 6, 6, 6, 6, 7, 6, 6, 6, 6, 6, 3, 3, 3, 3, 7, 6, 6, 6, 6, 6, 6, 5, 6, 6, 6, 6, 6, 6, 4, 6,
    5, 5, 5, 5, 5, 5, 9, 5, 5, 5, 5, 5, 2, 2, 3, 3, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 3, 5,
];

/// Returns the source cell of a glyph within the atlas.
///
/// Glyphs are packed left-to-right, wrapping to the next atlas line whenever a
/// glyph would cross the 128-pixel edge, exactly as raylib lays out its default
/// font. The returned tuple carries the top-left atlas coordinate and the
/// glyph's width.
fn glyph_source(index: usize) -> (usize, usize, usize) {
    let mut current_line = 0usize;
    let mut test_pos_x = GLYPH_DIVISOR;
    let mut cell_x = GLYPH_DIVISOR;
    let mut cell_y = GLYPH_DIVISOR;
    for &width in CHAR_WIDTHS.iter().take(index + 1) {
        let width = width as usize;
        cell_x = test_pos_x;
        cell_y = GLYPH_DIVISOR + current_line * (GLYPH_HEIGHT + GLYPH_DIVISOR);
        test_pos_x += width + GLYPH_DIVISOR;
        if test_pos_x >= ATLAS_SIZE {
            current_line += 1;
            test_pos_x = DOUBLE_DIVISOR + width;
            cell_x = GLYPH_DIVISOR;
            cell_y = GLYPH_DIVISOR + current_line * (GLYPH_HEIGHT + GLYPH_DIVISOR);
        }
    }
    (cell_x, cell_y, CHAR_WIDTHS[index] as usize)
}

/// Tests a single pixel of the atlas at `(x, y)`.
fn atlas_pixel_is_set(x: usize, y: usize) -> bool {
    let pixel_index = y * ATLAS_SIZE + x;
    let word = FONT_ATLAS[pixel_index / 32];
    (word >> (pixel_index % 32)) & 1 == 1
}

impl<'a> Canvas<'a> {
    /// Draws a string of text at `(x, y)` using the built-in bitmap font.
    ///
    /// `scale` scales the glyph pixels as integer multiples. Characters outside
    /// the font's 32..255 range are silently skipped.
    pub fn draw_text(&mut self, string: &str, x: i32, y: i32, scale: i32, color: Color) {
        let scale = scale.max(1);
        let mut cursor_x = x;
        for &byte in string.as_bytes() {
            if byte < FIRST_CHAR {
                continue;
            }
            // For a byte, the index is always below GLYPH_COUNT (255 - 32 = 223).
            let index = (byte - FIRST_CHAR) as usize;
            let (source_x, source_y, width) = glyph_source(index);
            for row_offset in 0..GLYPH_HEIGHT {
                for col_offset in 0..width {
                    if atlas_pixel_is_set(source_x + col_offset, source_y + row_offset) {
                        let block_x = cursor_x + (col_offset as i32) * scale;
                        let block_y = y + (row_offset as i32) * scale;
                        self.rect_filled(block_x, block_y, scale, scale, color);
                    }
                }
            }
            cursor_x += (width as i32) * scale;
        }
    }

    /// Returns the pixel width a string occupies at the given scale.
    pub fn measure_text(&self, string: &str, scale: i32) -> i32 {
        let scale = scale.max(1);
        let units = string
            .as_bytes()
            .iter()
            .filter(|&&byte| byte >= FIRST_CHAR)
            .map(|&byte| CHAR_WIDTHS[(byte - FIRST_CHAR) as usize] as i32)
            .sum::<i32>();
        units * scale
    }
}

#[cfg(test)]
mod tests {
    use super::CHAR_WIDTHS;
    use crate::canvas::{display, Canvas};
    use crate::color::palette;

    #[test]
    fn measure_scales_linearly() {
        let mut storage = vec![0u16; display::PIXEL_COUNT];
        let canvas = Canvas::new(&mut storage);
        assert_eq!(
            canvas.measure_text("AB", 1),
            (CHAR_WIDTHS['A' as usize - 32] + CHAR_WIDTHS['B' as usize - 32]) as i32
        );
        assert_eq!(
            canvas.measure_text("AB", 2),
            canvas.measure_text("AB", 1) * 2
        );
    }

    #[test]
    fn draw_text_paints_glyph_pixels() {
        let mut storage = vec![0u16; display::PIXEL_COUNT];
        let mut canvas = Canvas::new(&mut storage);
        canvas.draw_text("Hi!", 10, 10, 2, palette::WHITE);
        let painted = storage
            .iter()
            .filter(|&&p| p == palette::WHITE.raw())
            .count();
        assert!(painted > 0, "draw_text painted no pixels");
    }

    #[test]
    fn control_characters_are_skipped() {
        let mut storage = vec![0u16; display::PIXEL_COUNT];
        let mut canvas = Canvas::new(&mut storage);
        // Control characters below 32 are outside the font's glyph range.
        canvas.draw_text("\x00\x01\x1f", 0, 0, 1, palette::WHITE);
        assert!(storage.iter().all(|&p| p == 0));
    }
}
