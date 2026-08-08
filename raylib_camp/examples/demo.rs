//! Host-side demo that renders a test scene and writes it to a BMP file.
//!
//! This lets the rasterizer be inspected without any hardware. Run with:
//! `cargo run -p raylib_camp --example demo`

use raylib_camp::canvas::Canvas;
use raylib_camp::color::palette;

fn colour565_to888(value: u16) -> (u8, u8, u8) {
    let red = (value >> 11) & 0x1f;
    let green = (value >> 5) & 0x3f;
    let blue = value & 0x1f;
    let r = ((red as u8) << 3) | ((red >> 2) as u8);
    let g = ((green as u8) << 2) | ((green >> 4) as u8);
    let b = ((blue as u8) << 3) | ((blue >> 2) as u8);
    (r, g, b)
}

fn write_bmp(path: &str, buffer: &[u16], width: usize, height: usize) {
    use std::fs::File;
    use std::io::Write;

    let row_padding = (4 - (width * 3) % 4) % 4;
    let image_size = (width * 3 + row_padding) * height;
    let file_size = 54 + image_size;
    let mut data = Vec::with_capacity(file_size);
    data.extend_from_slice(b"BM");
    data.extend_from_slice(&(file_size as u32).to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&54u32.to_le_bytes());
    data.extend_from_slice(&40u32.to_le_bytes());
    data.extend_from_slice(&(width as i32).to_le_bytes());
    data.extend_from_slice(&(height as i32).to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&24u16.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&(image_size as u32).to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    for row in (0..height).rev() {
        for column in 0..width {
            let (r, g, b) = colour565_to888(buffer[row * width + column]);
            data.push(b);
            data.push(g);
            data.push(r);
        }
        data.extend(core::iter::repeat_n(0u8, row_padding));
    }
    let mut file = File::create(path).expect("open output file for writing");
    file.write_all(&data).expect("write bitmap data");
}

fn main() {
    let width = 240usize;
    let height = 240usize;
    let mut storage = vec![0u16; width * height];
    let mut canvas = Canvas::new(&mut storage);

    canvas.clear(palette::BLACK);
    canvas.rect_filled(0, 0, 240, 14, palette::BLUE);
    canvas.draw_text("CAMP 2026", 60, 2, 1, palette::WHITE);

    // Shape gallery.
    canvas.rect(10, 24, 60, 60, palette::RED);
    canvas.rect_filled(80, 24, 60, 60, palette::GREEN);
    canvas.circle(170, 54, 30, palette::YELLOW);
    canvas.circle_filled(40, 120, 24, palette::CYAN);
    canvas.triangle((90, 195), (150, 195), (120, 140), palette::MAGENTA);
    canvas.line(160, 120, 230, 120, palette::ORANGE);
    canvas.triangle_filled((170, 190), (230, 190), (200, 150), palette::GREY);

    // A few scattered points and a diagonal.
    for i in 0..20 {
        canvas.plot(10 + i, 210 + i, palette::WHITE);
    }

    let path = "target/demo.bmp";
    write_bmp(path, &storage, width, height);
    println!("wrote {path}");
}
