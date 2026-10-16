/// The R glyph shared by the tray, Windows window, and executable resource.
/// Only the black glyph is opaque; the background stays transparent.
pub fn rgba(size: u32) -> Vec<u8> {
    let glyph = [
        "11110", "10001", "10001", "11110", "10100", "10010", "10001",
    ];
    let mut rgba = vec![0; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let px = (x * 32 / size) as usize;
            let py = (y * 32 / size) as usize;
            let pixel = &mut rgba[((y * size + x) * 4) as usize..][..4];
            if (8..23).contains(&px)
                && (5..26).contains(&py)
                && glyph[(py - 5) / 3].as_bytes()[(px - 8) / 3] == b'1'
            {
                pixel.copy_from_slice(&[0, 0, 0, 255]);
            }
        }
    }
    rgba
}
