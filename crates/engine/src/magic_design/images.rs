use image::{Rgba, RgbaImage};

struct SpriteImage {
    image: RgbaImage,
}

/// Shared silhouette decontamination used by the native preview and mesh export.
pub fn decontaminate_sprite_rgba(
    src: &RgbaImage,
    target_height: Option<u32>,
) -> (RgbaImage, [u32; 4]) {
    SpriteImage { image: src.clone() }.decontaminate(target_height)
}

impl SpriteImage {
    fn decontaminate(mut self, target_height: Option<u32>) -> (RgbaImage, [u32; 4]) {
        let w = self.image.width().max(1);
        let h = self.image.height().max(1);
        let shadow_band_start = match self.alpha_bounds(64) {
            Some([_, min_y, _, max_y]) if max_y > min_y => min_y + ((max_y - min_y) * 86) / 100,
            _ => h,
        };
        self.remove_matte(shadow_band_start);
        let Some([bx0, by0, bx1, by1]) = self.alpha_bounds(96) else {
            return (self.image, [0, 0, w, h]);
        };
        if let Some(th) = target_height.filter(|&th| th > h + 16) {
            let scale = f64::from(th) / f64::from(h);
            let tw = ((f64::from(w) * scale).round() as u32).max(1);
            let resized = image::imageops::resize(
                &self.image,
                tw,
                th,
                image::imageops::FilterType::CatmullRom,
            );
            let sbx0 = ((f64::from(bx0) * scale).floor() as u32).min(tw.saturating_sub(1));
            let sby0 = ((f64::from(by0) * scale).floor() as u32).min(th.saturating_sub(1));
            let sbx1 = ((f64::from(bx1) * scale).ceil() as u32).min(tw.saturating_sub(1));
            let sby1 = ((f64::from(by1) * scale).ceil() as u32).min(th.saturating_sub(1));
            return (resized, [sbx0, sby0, sbx1, sby1]);
        }
        (self.image, [bx0, by0, bx1, by1])
    }

    fn alpha_bounds(&self, threshold: u8) -> Option<[u32; 4]> {
        let image = &self.image;
        let [mut x0, mut y0, mut x1, mut y1] = [image.width().max(1), image.height().max(1), 0, 0];
        for y in 0..image.height().max(1) {
            for x in 0..image.width().max(1) {
                if image.get_pixel(x, y)[3] >= threshold {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
        }
        (x1 >= x0 && y1 >= y0).then_some([x0, y0, x1, y1])
    }

    fn remove_matte(&mut self, shadow_band_start: u32) {
        for (_, y, pixel) in self.image.enumerate_pixels_mut() {
            let [r, g, b, a] = pixel.0;
            let max_c = r.max(g).max(b);
            let saturation = max_c.saturating_sub(r.min(g).min(b));
            let floor_shadow = y >= shadow_band_start && a < 215 && max_c < 98 && saturation < 24;
            if a < 28 || floor_shadow {
                *pixel = Rgba([0, 0, 0, 0]);
            }
        }
    }
}
