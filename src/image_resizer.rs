use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, imageops};
use std::io::Cursor;

pub struct ImageResizer {
    debug: bool,
}

pub struct SnsLimits {
    pub max_file_size: usize,
    pub max_width: u32,
    pub max_height: u32,
    pub default_quality: u8,
}

impl ImageResizer {
    pub fn new(debug: bool) -> Self {
        Self { debug }
    }

    fn debug_print(&self, message: &str) {
        if self.debug {
            println!("[DEBUG] ImageResizer: {}", message);
        }
    }

    pub fn get_sns_limits(&self, sns_type: &str) -> SnsLimits {
        match sns_type {
            "bluesky" => SnsLimits {
                max_file_size: 1024 * 1024, // 1MB
                max_width: 2000,
                max_height: 2000,
                default_quality: 85,
            },
            "x" => SnsLimits {
                max_file_size: 5 * 1024 * 1024, // 5MB
                max_width: 4096,
                max_height: 4096,
                default_quality: 85,
            },
            "mastodon" => SnsLimits {
                max_file_size: 10 * 1024 * 1024, // 10MB
                max_width: 1920,
                max_height: 1920,
                default_quality: 85,
            },
            "misskey" => SnsLimits {
                max_file_size: 10 * 1024 * 1024, // 10MB
                max_width: 2048,
                max_height: 2048,
                default_quality: 85,
            },
            _ => SnsLimits {
                max_file_size: 1024 * 1024, // デフォルトはBluesky基準
                max_width: 2000,
                max_height: 2000,
                default_quality: 85,
            },
        }
    }

    pub fn resize_image_data(&self, image_data: &[u8], sns_type: &str) -> anyhow::Result<Vec<u8>> {
        let limits = self.get_sns_limits(sns_type);
        self.debug_print(&format!(
            "開始: {}, 元サイズ: {} bytes",
            sns_type,
            image_data.len()
        ));

        // 入力フォーマットを判定する(JPEG/PNG以外はJPEGへ再エンコードする方針のため)
        let format = image::guess_format(image_data).ok();

        // デコードする。失敗した場合(対応外形式: AVIF等)は元データを返さずエラーにする。
        // 元バイトをそのまま送るとSNS側で「media type unrecognized」等で弾かれ、
        // 何が起きたか分かりにくくなるため、ここで明確に失敗させて呼び出し側で
        // 画像なし投稿へフォールバックさせる。
        let img = image::load_from_memory(image_data).map_err(|e| {
            self.debug_print(&format!("画像デコード失敗: {}", e));
            anyhow::anyhow!("画像のデコードに失敗しました(対応外の形式の可能性): {}", e)
        })?;

        let width = img.width();
        let height = img.height();

        // JPEG/PNGで上限内ならそのまま返す。各SNSのupload_mimeはpng/jpegはそのまま、
        // それ以外はimage/jpegとして送るため、この2形式のみ無変換が安全。
        let is_jpeg_or_png = matches!(
            format,
            Some(image::ImageFormat::Jpeg) | Some(image::ImageFormat::Png)
        );
        if is_jpeg_or_png
            && image_data.len() <= limits.max_file_size
            && width <= limits.max_width
            && height <= limits.max_height
        {
            self.debug_print("変換不要(JPEG/PNGかつ上限内)");
            return Ok(image_data.to_vec());
        }

        // ここから先は常にJPEGとして出力する(WebP等の非JPEG/PNG、または上限超過時)。
        self.debug_print(&format!(
            "JPEGへ再エンコード: {}x{}, 形式: {:?}, color: {:?}",
            width,
            height,
            format,
            img.color()
        ));

        // 2. RGBA画像等の場合は白背景に重ねてRGBにする
        let rgb_img = match img {
            DynamicImage::ImageRgba8(rgba_img) => {
                let mut base = image::ImageBuffer::from_pixel(
                    rgba_img.width(),
                    rgba_img.height(),
                    image::Rgba([255, 255, 255, 255]),
                );
                imageops::overlay(&mut base, &rgba_img, 0, 0);
                DynamicImage::ImageRgba8(base).to_rgb8()
            }
            DynamicImage::ImageLumaA8(luma_a_img) => {
                let rgba_img = DynamicImage::ImageLumaA8(luma_a_img).to_rgba8();
                let mut base = image::ImageBuffer::from_pixel(
                    rgba_img.width(),
                    rgba_img.height(),
                    image::Rgba([255, 255, 255, 255]),
                );
                imageops::overlay(&mut base, &rgba_img, 0, 0);
                DynamicImage::ImageRgba8(base).to_rgb8()
            }
            other => other.to_rgb8(),
        };
        let mut rgb_img = DynamicImage::ImageRgb8(rgb_img);

        // 3. アスペクト比を維持して最大縦横に収まるよう縮小する
        if rgb_img.width() > limits.max_width || rgb_img.height() > limits.max_height {
            rgb_img = rgb_img.thumbnail(limits.max_width, limits.max_height);
            self.debug_print(&format!(
                "サイズ調整後: {}x{}",
                rgb_img.width(),
                rgb_img.height()
            ));
        }

        // 4. 品質を段階的に下げて圧縮を試みる
        let mut result_data = Vec::new();
        let mut current_quality = limits.default_quality;

        while current_quality >= 30 {
            let mut buf = Vec::new();
            let mut cursor = Cursor::new(&mut buf);
            let encoder = JpegEncoder::new_with_quality(&mut cursor, current_quality);
            if let Err(e) = rgb_img.write_with_encoder(encoder) {
                self.debug_print(&format!("JPEGエンコードエラー: {}", e));
                return Ok(image_data.to_vec());
            }
            result_data = buf;
            self.debug_print(&format!(
                "品質{}: {} bytes",
                current_quality,
                result_data.len()
            ));

            if result_data.len() <= limits.max_file_size {
                self.debug_print(&format!(
                    "完了: {} bytes (品質: {})",
                    result_data.len(),
                    current_quality
                ));
                return Ok(result_data);
            }

            if current_quality <= 30 {
                break;
            }
            current_quality = current_quality.saturating_sub(5);
        }

        // 5. 最低品質(30)でも超える場合は、さらにスケールを縮小
        let mut scale_factor = 0.9f32;
        while result_data.len() > limits.max_file_size && scale_factor > 0.5 {
            let new_width = (rgb_img.width() as f32 * scale_factor) as u32;
            let new_height = (rgb_img.height() as f32 * scale_factor) as u32;

            let resized = rgb_img.resize(new_width, new_height, imageops::FilterType::CatmullRom);

            let mut buf = Vec::new();
            let mut cursor = Cursor::new(&mut buf);
            let encoder = JpegEncoder::new_with_quality(&mut cursor, 30);
            if let Err(e) = resized.write_with_encoder(encoder) {
                self.debug_print(&format!("縮小時JPEGエンコードエラー: {}", e));
                return Ok(image_data.to_vec());
            }
            result_data = buf;
            self.debug_print(&format!(
                "追加縮小 {:.1}: {}x{}, {} bytes",
                scale_factor,
                new_width,
                new_height,
                result_data.len()
            ));

            if result_data.len() <= limits.max_file_size {
                self.debug_print(&format!(
                    "完了: {} bytes (スケール: {:.1})",
                    result_data.len(),
                    scale_factor
                ));
                return Ok(result_data);
            }

            scale_factor -= 0.1;
        }

        self.debug_print(&format!("最終: {} bytes", result_data.len()));
        Ok(result_data)
    }

    #[allow(dead_code)]
    pub fn resize_image_file(&self, file_path: &str, sns_type: &str) -> anyhow::Result<Vec<u8>> {
        let original_data = std::fs::read(file_path)?;
        self.resize_image_data(&original_data, sns_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    fn create_dummy_image(width: u32, height: u32) -> Vec<u8> {
        let img = ImageBuffer::from_fn(width, height, |_, _| Rgb([255, 0, 0]));
        let dynamic_img = DynamicImage::ImageRgb8(img);
        let mut buf = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buf);
        dynamic_img
            .write_to(&mut cursor, image::ImageFormat::Jpeg)
            .unwrap();
        buf
    }

    #[test]
    fn test_resize_no_need() {
        let resizer = ImageResizer::new(false);
        let data = create_dummy_image(100, 100);
        let result = resizer.resize_image_data(&data, "bluesky").unwrap();
        assert_eq!(result.len(), data.len());
    }

    #[test]
    fn test_resize_large_image() {
        let resizer = ImageResizer::new(false);
        let data = create_dummy_image(3000, 3000);
        let result = resizer.resize_image_data(&data, "bluesky").unwrap();

        let img = image::load_from_memory(&result).unwrap();
        assert!(img.width() <= 2000);
        assert!(img.height() <= 2000);
        assert!(result.len() <= 1024 * 1024);
    }

    /// JPEG/PNG以外(ここではBMP)は上限内でもJPEGへ変換されることを確認する。
    /// upload_mimeがimage/jpegとして送るため、実体もJPEGである必要がある。
    #[test]
    fn test_non_jpeg_png_converted_to_jpeg() {
        let resizer = ImageResizer::new(false);
        // 100x100の小さなBMP(上限内)を作る
        let img = ImageBuffer::from_fn(100, 100, |_, _| Rgb([0, 128, 255]));
        let dynamic_img = DynamicImage::ImageRgb8(img);
        let mut data = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut data);
        dynamic_img
            .write_to(&mut cursor, image::ImageFormat::Bmp)
            .unwrap();
        assert_eq!(image::guess_format(&data).unwrap(), image::ImageFormat::Bmp);

        let result = resizer.resize_image_data(&data, "x").unwrap();
        // 出力はJPEGになっている
        assert_eq!(
            image::guess_format(&result).unwrap(),
            image::ImageFormat::Jpeg
        );
    }

    /// デコードできない(対応外形式の)データはエラーになり、
    /// 元バイトをそのまま返さないことを確認する。
    #[test]
    fn test_undecodable_returns_err() {
        let resizer = ImageResizer::new(false);
        let garbage = vec![0u8, 1, 2, 3, 4, 5, 6, 7];
        let result = resizer.resize_image_data(&garbage, "x");
        assert!(result.is_err());
    }
}
