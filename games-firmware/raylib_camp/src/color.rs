//! 16-bit RGB565 colour representation and the badge's named palette.

/// A 16-bit RGB565 colour as stored by the GC9A01A display.
///
/// Bits 15..11 are red, bits 10..5 are green and bits 4..0 are blue.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Color(u16);

impl Color {
    /// Constructs a colour from 8-bit-per-channel red, green and blue.
    pub const fn rgb565(red: u8, green: u8, blue: u8) -> Self {
        let red5 = (red >> 3) as u16;
        let green6 = (green >> 2) as u16;
        let blue5 = (blue >> 3) as u16;
        Color((red5 << 11) | (green6 << 5) | blue5)
    }

    /// Wraps a raw 16-bit RGB565 value.
    pub const fn from_raw(value: u16) -> Self {
        Color(value)
    }

    /// Returns the underlying 16-bit RGB565 value.
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// Returns the 5-bit red component.
    pub const fn red(self) -> u8 {
        ((self.0 >> 11) & 0x1f) as u8
    }

    /// Returns the 6-bit green component.
    pub const fn green(self) -> u8 {
        ((self.0 >> 5) & 0x3f) as u8
    }

    /// Returns the 5-bit blue component.
    pub const fn blue(self) -> u8 {
        (self.0 & 0x1f) as u8
    }
}

/// Common colours available without constructing them by hand.
pub mod palette {
    use super::Color;

    /// Pitch black.
    pub const BLACK: Color = Color::rgb565(0x00, 0x00, 0x00);
    /// Full white.
    pub const WHITE: Color = Color::rgb565(0xff, 0xff, 0xff);
    /// Red.
    pub const RED: Color = Color::rgb565(0xff, 0x00, 0x00);
    /// Green.
    pub const GREEN: Color = Color::rgb565(0x00, 0xff, 0x00);
    /// Blue.
    pub const BLUE: Color = Color::rgb565(0x00, 0x00, 0xff);
    /// Yellow.
    pub const YELLOW: Color = Color::rgb565(0xff, 0xff, 0x00);
    /// Cyan.
    pub const CYAN: Color = Color::rgb565(0x00, 0xff, 0xff);
    /// Magenta.
    pub const MAGENTA: Color = Color::rgb565(0xff, 0x00, 0xff);
    /// Orange.
    pub const ORANGE: Color = Color::rgb565(0xff, 0x80, 0x00);
    /// Mid grey.
    pub const GREY: Color = Color::rgb565(0x80, 0x80, 0x80);
}

#[cfg(test)]
mod tests {
    use super::Color;

    #[test]
    fn components_recombine_to_the_original_value() {
        for value in [0x0000u16, 0xffff, 0x8000, 0x07e0, 0x001f] {
            let color = Color::from_raw(value);
            let rebuilt = ((color.red() as u16) << 11)
                | ((color.green() as u16) << 5)
                | (color.blue() as u16);
            assert_eq!(
                rebuilt, value,
                "components did not recombine to {value:#06x}"
            );
        }
    }

    #[test]
    fn named_primary_colours_match_expected_bit_patterns() {
        use super::palette;
        assert_eq!(palette::BLACK.raw(), 0x0000);
        assert_eq!(palette::WHITE.raw(), 0xffff);
        assert_eq!(palette::RED.raw(), 0xf800);
        assert_eq!(palette::GREEN.raw(), 0x07e0);
        assert_eq!(palette::BLUE.raw(), 0x001f);
    }
}
