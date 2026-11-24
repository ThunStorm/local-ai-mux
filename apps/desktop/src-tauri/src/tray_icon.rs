use std::sync::LazyLock;
use tauri::image::Image;

const TEMPLATE: Image<'static> = tauri::include_image!("icons/tray-template.png");
const RED: [u8; 3] = [235, 36, 32];

// The detached sun is the component containing this point in the selected asset.
static SUN: LazyLock<Vec<bool>> = LazyLock::new(|| {
    let width = TEMPLATE.width() as usize;
    let height = TEMPLATE.height() as usize;
    let mut mask = vec![false; width * height];
    let mut pending = vec![(width * 4 / 5, height / 4)];
    while let Some((x, y)) = pending.pop() {
        let index = y * width + x;
        if mask[index] || TEMPLATE.rgba()[index * 4 + 3] < 128 {
            continue;
        }
        mask[index] = true;
        if x > 0 {
            pending.push((x - 1, y));
        }
        if x + 1 < width {
            pending.push((x + 1, y));
        }
        if y > 0 {
            pending.push((x, y - 1));
        }
        if y + 1 < height {
            pending.push((x, y + 1));
        }
    }
    mask
});

pub fn icon(running: bool, foreground: [u8; 3]) -> Image<'static> {
    if !running {
        return TEMPLATE.clone();
    }
    let mut rgba = TEMPLATE.rgba().to_vec();
    for (index, pixel) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        pixel[..3].copy_from_slice(if SUN[index] { &RED } else { &foreground });
    }
    Image::new_owned(rgba, TEMPLATE.width(), TEMPLATE.height())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_colors_only_the_detached_sun_and_preserves_transparency() {
        let count = SUN.iter().filter(|&&sun| sun).count();
        assert!(
            (40..130).contains(&count),
            "sun must stay a small detached component: {count}"
        );
        for foreground in [[255; 3], [0; 3]] {
            let image = icon(true, foreground);
            assert_eq!((image.width(), image.height()), (72, 42));
            for (index, (pixel, source)) in image
                .rgba()
                .as_chunks::<4>()
                .0
                .iter()
                .zip(TEMPLATE.rgba().as_chunks::<4>().0.iter())
                .enumerate()
            {
                assert_eq!(pixel[3], source[3]);
                if SUN[index] {
                    assert!(index % 72 >= 50 && index / 72 < 18);
                    assert_eq!(&pixel[..3], &RED);
                } else if pixel[3] != 0 {
                    assert_eq!(&pixel[..3], &foreground);
                }
            }
        }
    }

    #[test]
    fn stopped_restores_the_original_template_after_running() {
        let _ = icon(true, [255; 3]);
        assert_eq!(icon(false, [255; 3]).rgba(), TEMPLATE.rgba());
        assert_eq!(icon(false, [0; 3]).rgba(), TEMPLATE.rgba());
        assert_eq!(TEMPLATE.rgba()[3], 0);
    }
}
