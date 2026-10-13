/// The R glyph shared by the tray, Windows window, and executable resource.
/// Template icons contain only the black glyph; app icons use a red tile.
pub fn rgba(size: u32, template: bool) -> Vec<u8> {
    let glyph = [
        "11110", "10001", "10001", "11110", "10100", "10010", "10001",
    ];
    let mut rgba = vec![0; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let px = (x * 32 / size) as usize;
            let py = (y * 32 / size) as usize;
            let pixel = &mut rgba[((y * size + x) * 4) as usize..][..4];
            if !template {
                let dx = 5usize.saturating_sub(px).max(px.saturating_sub(26));
                let dy = 5usize.saturating_sub(py).max(py.saturating_sub(26));
                if dx * dx + dy * dy <= 25 {
                    pixel.copy_from_slice(&[220, 38, 38, 255]);
                }
            }
            if (8..23).contains(&px)
                && (5..26).contains(&py)
                && glyph[(py - 5) / 3].as_bytes()[(px - 8) / 3] == b'1'
            {
                pixel.copy_from_slice(if template {
                    &[0, 0, 0, 255]
                } else {
                    &[255, 255, 255, 255]
                });
            }
        }
    }
    rgba
}
