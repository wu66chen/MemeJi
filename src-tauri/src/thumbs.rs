//! 缩略图管道：动图首帧解码、等比缩放、content-hash 命名落盘缓存、异步去重队列。
//!
//! Seam：`decode_first_frame` / `scaled_dimensions` / `generate_thumbnail` /
//! `ensure_thumbnail` / `enqueue` / `thumbnail_path` / `cache_dir`。

use crate::AppState;
use image::AnimationDecoder;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};

/// 缩略图统一规格：一档 PNG，最长边 256px。
pub const THUMB_MAX_EDGE: u32 = 256;

pub const THUMBNAIL_READY_EVENT: &str = "thumbnail-ready";

#[derive(Debug)]
pub struct ThumbError(pub String);

impl std::fmt::Display for ThumbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ThumbError {}
impl From<image::ImageError> for ThumbError {
    fn from(e: image::ImageError) -> Self {
        ThumbError(e.to_string())
    }
}
impl From<std::io::Error> for ThumbError {
    fn from(e: std::io::Error) -> Self {
        ThumbError(e.to_string())
    }
}
impl From<rusqlite::Error> for ThumbError {
    fn from(e: rusqlite::Error) -> Self {
        ThumbError(e.to_string())
    }
}

pub fn cache_dir(library_root: &Path) -> PathBuf {
    library_root.join("thumbs")
}

/// 缓存键 = content hash：`{hash}.png`，同 hash 多张图共享同一份文件。
pub fn thumbnail_path(cache_dir: &Path, content_hash: &str) -> PathBuf {
    cache_dir.join(format!("{content_hash}.png"))
}

/// 缩放输出尺寸：最长边 ≤ max_edge，保持宽高比，小图不放大。
pub fn scaled_dimensions(width: u32, height: u32, max_edge: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest == 0 || longest <= max_edge {
        return (width, height);
    }
    let scale = max_edge as f64 / longest as f64;
    (
        (((width as f64) * scale).round() as u32).max(1),
        (((height as f64) * scale).round() as u32).max(1),
    )
}

/// 解码出用于缩略图的图像：GIF / 动态 WebP / APNG 取首帧，静态图整图。
pub fn decode_first_frame(bytes: &[u8]) -> Result<image::DynamicImage, ThumbError> {
    let reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let format = reader
        .format()
        .ok_or_else(|| ThumbError("未知图片格式".into()))?;
    match format {
        image::ImageFormat::Gif => {
            let decoder = image::codecs::gif::GifDecoder::new(reader.into_inner())?;
            first_frame(decoder.into_frames())
        }
        image::ImageFormat::Png => {
            let decoder = image::codecs::png::PngDecoder::new(reader.into_inner())?;
            if decoder.is_apng().unwrap_or(false) {
                first_frame(decoder.apng()?.into_frames())
            } else {
                Ok(image::DynamicImage::from_decoder(decoder)?)
            }
        }
        image::ImageFormat::WebP => {
            let decoder = image::codecs::webp::WebPDecoder::new(reader.into_inner())?;
            if decoder.has_animation() {
                first_frame(decoder.into_frames())
            } else {
                Ok(image::DynamicImage::from_decoder(decoder)?)
            }
        }
        _ => Ok(reader.decode()?),
    }
}

fn first_frame(
    mut frames: impl Iterator<Item = image::ImageResult<image::Frame>>,
) -> Result<image::DynamicImage, ThumbError> {
    let frame = frames
        .next()
        .transpose()?
        .ok_or_else(|| ThumbError("没有可解码的帧".into()))?;
    Ok(image::DynamicImage::ImageRgba8(frame.buffer().clone()))
}

/// 判定是否动图（GIF / 动态 WebP / APNG）。Smart Copy 据此选择
/// 「位图通道」或「文件引用 + 首帧兜底」策略。
pub fn detect_animation(bytes: &[u8]) -> Result<bool, ThumbError> {
    let reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let format = reader
        .format()
        .ok_or_else(|| ThumbError("未知图片格式".into()))?;
    match format {
        image::ImageFormat::Gif => Ok(true), // GIF 一律走文件引用，保动画
        image::ImageFormat::Png => {
            let decoder = image::codecs::png::PngDecoder::new(reader.into_inner())?;
            Ok(decoder.is_apng().unwrap_or(false))
        }
        image::ImageFormat::WebP => {
            let decoder = image::codecs::webp::WebPDecoder::new(reader.into_inner())?;
            Ok(decoder.has_animation())
        }
        _ => Ok(false),
    }
}

#[derive(Debug)]
pub struct GeneratedThumbnail {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
}

/// 生成并落盘缩略图（不查缓存，调用方决定是否走 `ensure_thumbnail`）。
pub fn generate_thumbnail(
    bytes: &[u8],
    cache_dir: &Path,
    content_hash: &str,
) -> Result<GeneratedThumbnail, ThumbError> {
    let img = decode_first_frame(bytes)?;
    let rgba = img.to_rgba8();
    let (src_w, src_h) = (rgba.width(), rgba.height());
    let (dst_w, dst_h) = scaled_dimensions(src_w, src_h, THUMB_MAX_EDGE);
    let resized = if (dst_w, dst_h) == (src_w, src_h) {
        rgba.clone()
    } else {
        let src = fast_image_resize::images::Image::from_vec_u8(
            src_w,
            src_h,
            rgba.into_raw(),
            fast_image_resize::PixelType::U8x4,
        )
        .map_err(|e| ThumbError(e.to_string()))?;
        let mut dst = fast_image_resize::images::Image::new(
            dst_w,
            dst_h,
            fast_image_resize::PixelType::U8x4,
        );
        let mut resizer = fast_image_resize::Resizer::new();
        let options = fast_image_resize::ResizeOptions::new().resize_alg(
            fast_image_resize::ResizeAlg::Convolution(fast_image_resize::FilterType::Bilinear),
        );
        resizer
            .resize(&src, &mut dst, &options)
            .map_err(|e| ThumbError(e.to_string()))?;
        image::RgbaImage::from_raw(dst_w, dst_h, dst.into_vec())
            .ok_or_else(|| ThumbError("缩放结果尺寸不匹配".into()))?
    };
    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(resized).write_to(&mut png, image::ImageFormat::Png)?;
    std::fs::create_dir_all(cache_dir)?;
    let path = thumbnail_path(cache_dir, content_hash);
    write_atomic(&path, png.get_ref())?;
    Ok(GeneratedThumbnail { path, width: dst_w, height: dst_h })
}

fn write_atomic(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = target.with_extension("thumb.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, target)
}

/// 命中缓存直接返回（零成本，不读原图）；miss 则按 hash 找到库内原图生成。
pub fn ensure_thumbnail(
    conn: &rusqlite::Connection,
    library_root: &Path,
    content_hash: &str,
) -> Result<PathBuf, ThumbError> {
    let dir = cache_dir(library_root);
    let cached = thumbnail_path(&dir, content_hash);
    if cached.exists() {
        return Ok(cached);
    }
    let internal = crate::library::find_internal_path(conn, content_hash)?
        .ok_or_else(|| ThumbError(format!("库中不存在 hash 为 {content_hash} 的表情")))?;
    let bytes = std::fs::read(library_root.join(internal))?;
    Ok(generate_thumbnail(&bytes, &dir, content_hash)?.path)
}

/// 异步去重队列：同 hash 只排一次；完成后向所有窗口广播 `thumbnail-ready`
/// （payload `{ hash, path }`，path 为 null 表示生成失败）。
pub fn enqueue(app: &AppHandle, state: &AppState, hashes: &[String]) {
    let mut newly = Vec::new();
    {
        let mut queue = state.thumb_queue.lock().unwrap();
        for hash in hashes {
            if queue.insert(hash.clone()) {
                newly.push(hash.clone());
            }
        }
    }
    for hash in newly {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let state = app.state::<AppState>();
            let library_root = state.library_root_clone();
            let outcome = {
                let conn = state.conn.lock().unwrap();
                ensure_thumbnail(&conn, &library_root, &hash)
            };
            // 无论成败都移出队列：失败不自动重试，避免坏图风暴
            state.thumb_queue.lock().unwrap().remove(&hash);
            let path = outcome.ok().map(|p| p.to_string_lossy().to_string());
            let _ = app.emit(
                THUMBNAIL_READY_EVENT,
                serde_json::json!({ "hash": hash, "path": path }),
            );
        });
    }
}

/// 供 `list_memes` 做缓存标记：已缓存返回绝对路径，未缓存返回 None。
pub fn cached_thumbnail_path(library_root: &Path, content_hash: &str) -> Option<String> {
    let path = thumbnail_path(&cache_dir(library_root), content_hash);
    path.exists().then(|| path.to_string_lossy().to_string())
}

// ---------------------------------------------------------------------------
// 测试：三格式首帧提取、缓存键、缩放输出尺寸、生成与零成本命中
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{self, open_db};
    use image::{AnimationDecoder, DynamicImage, ImageBuffer, ImageEncoder, Rgba, RgbaImage};
    use std::io::Cursor;

    fn solid(width: u32, height: u32, color: [u8; 3]) -> RgbaImage {
        ImageBuffer::from_fn(width, height, |_, _| Rgba([color[0], color[1], color[2], 255]))
    }

    fn encode_png(img: &RgbaImage) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img.clone())
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    /// 两帧不同色的 GIF（用 image 自带的 GifEncoder 逐帧编码）。
    fn two_frame_gif() -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(&mut buf);
            let delay = image::Delay::from_numer_denom_ms(100, 1000);
            encoder
                .encode_frame(image::Frame::from_parts(solid(16, 16, [255, 0, 0]), 0, 0, delay))
                .unwrap();
            encoder
                .encode_frame(image::Frame::from_parts(solid(16, 16, [0, 0, 255]), 0, 0, delay))
                .unwrap();
        }
        buf.into_inner()
    }

    fn png_chunks(file: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
        let mut out = Vec::new();
        let mut pos = 8; // 跳过 PNG 签名
        while pos + 8 <= file.len() {
            let len = u32::from_be_bytes(file[pos..pos + 4].try_into().unwrap()) as usize;
            let tag: [u8; 4] = file[pos + 4..pos + 8].try_into().unwrap();
            out.push((tag, file[pos + 8..pos + 8 + len].to_vec()));
            pos += 12 + len;
        }
        out
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut table = [0u32; 256];
        for i in 0..256u32 {
            let mut c = i;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            table[i as usize] = c;
        }
        let mut crc = 0xFFFF_FFFFu32;
        for &b in data {
            crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
        }
        crc ^ 0xFFFF_FFFF
    }

    fn png_chunk(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut v = Vec::with_capacity(12 + data.len());
        v.extend_from_slice(&(data.len() as u32).to_be_bytes());
        v.extend_from_slice(tag);
        v.extend_from_slice(data);
        let mut crc_input = Vec::with_capacity(4 + data.len());
        crc_input.extend_from_slice(tag);
        crc_input.extend_from_slice(data);
        v.extend_from_slice(&crc32(&crc_input).to_be_bytes());
        v
    }

    /// 两帧不同色的 APNG：image 0.25 没有 APNG 编码器，
    /// 用两个普通 PNG 的 IHDR/IDAT 手工组装 acTL/fcTL/fdAT 容器。
    fn two_frame_apng() -> Vec<u8> {
        let red = encode_png(&solid(16, 16, [255, 0, 0]));
        let blue = encode_png(&solid(16, 16, [0, 0, 255]));
        let idat_of = |file: &[u8]| -> Vec<u8> {
            png_chunks(file)
                .into_iter()
                .filter(|(t, _)| t == b"IDAT")
                .flat_map(|(_, d)| d)
                .collect()
        };
        let ihdr = png_chunks(&red).into_iter().find(|(t, _)| t == b"IHDR").unwrap().1;

        let mut actl = Vec::new();
        actl.extend_from_slice(&2u32.to_be_bytes()); // 帧数
        actl.extend_from_slice(&0u32.to_be_bytes()); // 无限循环
        let fctl = |seq: u32| -> Vec<u8> {
            let mut v = Vec::new();
            v.extend_from_slice(&seq.to_be_bytes());
            v.extend_from_slice(&16u32.to_be_bytes()); // 宽
            v.extend_from_slice(&16u32.to_be_bytes()); // 高
            v.extend_from_slice(&0u32.to_be_bytes()); // x
            v.extend_from_slice(&0u32.to_be_bytes()); // y
            v.extend_from_slice(&100u16.to_be_bytes()); // 时长分子
            v.extend_from_slice(&1000u16.to_be_bytes()); // 时长分母
            v.push(0); // 保留画面
            v.push(0); // 直接替换（不混合）
            v
        };
        let mut fdat = Vec::new();
        fdat.extend_from_slice(&2u32.to_be_bytes()); // 序号
        fdat.extend_from_slice(&idat_of(&blue));

        let mut out = Vec::new();
        out.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        out.extend_from_slice(&png_chunk(b"IHDR", &ihdr));
        out.extend_from_slice(&png_chunk(b"acTL", &actl));
        out.extend_from_slice(&png_chunk(b"fcTL", &fctl(0)));
        out.extend_from_slice(&png_chunk(b"IDAT", &idat_of(&red)));
        out.extend_from_slice(&png_chunk(b"fcTL", &fctl(1)));
        out.extend_from_slice(&png_chunk(b"fdAT", &fdat));
        out.extend_from_slice(&png_chunk(b"IEND", &[]));
        out
    }

    fn riff_chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::with_capacity(8 + payload.len() + 1);
        v.extend_from_slice(tag);
        v.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        v.extend_from_slice(payload);
        if payload.len() % 2 == 1 {
            v.push(0);
        }
        v
    }

    fn encode_lossless_webp(img: &RgbaImage) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut buf);
            encoder
                .write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgba8)
                .unwrap();
        }
        buf.into_inner()
    }

    /// image 只能编码单帧 WebP，这里手工把两个单帧 VP8L 装进动画 WebP 容器。
    fn two_frame_webp() -> Vec<u8> {
        let frame_payload = |webp_file: &[u8]| -> Vec<u8> {
            // 从单帧 WebP 文件（RIFF 容器）中取出第一个 VP8L 载荷
            let mut pos = 12usize; // "RIFF" + size + "WEBP"
            loop {
                let tag = &webp_file[pos..pos + 4];
                let size = u32::from_le_bytes(webp_file[pos + 4..pos + 8].try_into().unwrap()) as usize;
                if tag == b"VP8L" || tag == b"VP8 " {
                    return webp_file[pos + 8..pos + 8 + size].to_vec();
                }
                pos += 8 + size + (size & 1);
            }
        };
        let u24 = |v: u32| -> [u8; 3] { (v.to_le_bytes()[..3]).try_into().unwrap() };
        let dims = |w: u32, h: u32, into: &mut Vec<u8>| {
            into.extend_from_slice(&u24(w - 1));
            into.extend_from_slice(&u24(h - 1));
        };

        let mut body = Vec::new();
        let mut vp8x = vec![0x02u8, 0, 0, 0]; // 标志位 bit1 = 有动画，后跟 3 个保留字节
        dims(16, 16, &mut vp8x);
        body.extend_from_slice(&riff_chunk(b"VP8X", &vp8x));
        body.extend_from_slice(&riff_chunk(b"ANIM", &[0, 0, 0, 0, 0, 0]));
        for color in [[255u8, 0, 0], [0, 0, 255]] {
            let mut anmf = Vec::new();
            anmf.extend_from_slice(&u24(0));
            anmf.extend_from_slice(&u24(0));
            dims(16, 16, &mut anmf);
            anmf.extend_from_slice(&u24(100)); // 时长 ms
            // bit1 = 不混合：整帧直接替换，避免 image-webp 的整数近似混合引入 ±1 误差
            anmf.push(0x02);
            let frame = frame_payload(&encode_lossless_webp(&solid(16, 16, color)));
            anmf.extend_from_slice(&riff_chunk(b"VP8L", &frame));
            body.extend_from_slice(&riff_chunk(b"ANMF", &anmf));
        }
        let mut riff = Vec::new();
        riff.extend_from_slice(b"RIFF");
        riff.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
        riff.extend_from_slice(b"WEBP");
        riff.extend_from_slice(&body);
        riff
    }

    #[test]
    fn gif_thumbnail_uses_first_frame() {
        let img = decode_first_frame(&two_frame_gif()).unwrap();
        let rgba = img.to_rgba8();
        assert_eq!((rgba.width(), rgba.height()), (16, 16));
        assert_eq!(rgba.get_pixel(0, 0), &Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn apng_thumbnail_uses_first_frame() {
        let img = decode_first_frame(&two_frame_apng()).unwrap();
        let rgba = img.to_rgba8();
        assert_eq!((rgba.width(), rgba.height()), (16, 16));
        assert_eq!(rgba.get_pixel(8, 8), &Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn animated_webp_thumbnail_uses_first_frame() {
        let bytes = two_frame_webp();
        // 自检：容器里确实有两帧，且第二帧是蓝色
        let mut frames = image::codecs::webp::WebPDecoder::new(Cursor::new(&bytes))
            .unwrap()
            .into_frames();
        let first = frames.next().unwrap().unwrap();
        assert_eq!(first.buffer().get_pixel(0, 0), &Rgba([255, 0, 0, 255]));
        let second = frames.next().unwrap().unwrap();
        assert_eq!(second.buffer().get_pixel(0, 0), &Rgba([0, 0, 255, 255]));

        let img = decode_first_frame(&bytes).unwrap();
        let rgba = img.to_rgba8();
        assert_eq!((rgba.width(), rgba.height()), (16, 16));
        assert_eq!(rgba.get_pixel(0, 0), &Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn static_png_decodes_whole_image() {
        let img = decode_first_frame(&encode_png(&solid(8, 8, [1, 2, 3]))).unwrap();
        assert_eq!((img.width(), img.height()), (8, 8));
    }

    #[test]
    fn cache_key_is_content_hash_filename_and_shared_by_identical_content() {
        let dir = Path::new("thumbs");
        let path = thumbnail_path(dir, "abc123");
        assert_eq!(path, Path::new("thumbs").join("abc123.png"));
        // 同 hash（即同内容）的多张图拿到同一个缓存文件
        assert_eq!(thumbnail_path(dir, "abc123"), thumbnail_path(dir, "abc123"));
        assert_ne!(thumbnail_path(dir, "abc123"), thumbnail_path(dir, "def456"));
    }

    #[test]
    fn scaled_dimensions_preserve_aspect_ratio_without_upscaling() {
        assert_eq!(scaled_dimensions(1024, 512, 256), (256, 128));
        assert_eq!(scaled_dimensions(512, 1024, 256), (128, 256));
        assert_eq!(scaled_dimensions(512, 512, 256), (256, 256));
        assert_eq!(scaled_dimensions(300, 299, 256), (256, 255));
        assert_eq!(scaled_dimensions(200, 100, 256), (200, 100)); // 小图不放大
        assert_eq!(scaled_dimensions(1, 10000, 256), (1, 256));
    }

    #[test]
    fn generated_thumbnail_lands_in_cache_named_by_hash() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let source = encode_png(&solid(512, 256, [9, 9, 9]));
        let hash = "feedc0de";

        let g = generate_thumbnail(&source, dir, hash).unwrap();

        assert_eq!(g.path, thumbnail_path(dir, hash));
        assert!(g.path.exists());
        let img = image::open(&g.path).unwrap();
        assert_eq!((img.width(), img.height()), (256, 128));
    }

    #[test]
    fn ensure_thumbnail_hits_cache_with_zero_cost() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let conn = open_db(&lib).unwrap();
        // 512x512 的 PNG：原图
        let src = tmp.path().join("big.png");
        std::fs::write(&src, encode_png(&solid(512, 512, [7, 7, 7]))).unwrap();
        library::import_paths(&conn, &lib, &[src.clone()]).unwrap();
        let hash = library::list_memes(&conn, &library::GalleryView::All).unwrap()[0]
            .content_hash
            .clone();

        let dir = cache_dir(&lib);
        // 第一次：miss，生成落盘
        let p1 = ensure_thumbnail(&conn, &lib, &hash).unwrap();
        assert_eq!(p1, thumbnail_path(&dir, &hash));
        assert!(p1.exists());
        let cached_bytes = std::fs::read(&p1).unwrap();

        // 第二次：连库内原图都被删掉也能命中缓存 → 证明零成本（不再读原图）
        std::fs::remove_file(&src).unwrap();
        let p2 = ensure_thumbnail(&conn, &lib, &hash).unwrap();
        assert_eq!(p2, p1);
        assert_eq!(std::fs::read(&p2).unwrap(), cached_bytes);
    }

    #[test]
    fn ensure_thumbnail_without_cache_or_source_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let conn = open_db(&lib).unwrap();
        let err = ensure_thumbnail(&conn, &lib, "nosuchhash").unwrap_err();
        assert!(err.0.contains("不存在"));
    }

    #[test]
    fn corrupt_source_fails_generation() {
        let tmp = tempfile::tempdir().unwrap();
        let err = generate_thumbnail(b"not an image", tmp.path(), "deadbeef").unwrap_err();
        let _ = err;
        assert!(!thumbnail_path(tmp.path(), "deadbeef").exists());
    }

    /// 性能预算（spec）：缩略图单张 <100ms（release 计）。debug 放宽 10 倍。
    #[test]
    fn perf_thumbnail_generation_within_budget() {
        let tmp = tempfile::tempdir().unwrap();
        // 1024×768 伪随机噪点：贴近真实解码/缩放成本
        let img = ImageBuffer::from_fn(1024, 768, |x, y| {
            Rgba([
                ((x * 13 + y * 7) % 256) as u8,
                ((x * 5 + y * 29) % 256) as u8,
                ((x * 61 + y * 3) % 256) as u8,
                255,
            ])
        });
        let mut png = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();

        let start = std::time::Instant::now();
        let g = generate_thumbnail(png.get_ref(), tmp.path(), "perftest").unwrap();
        let elapsed = start.elapsed();
        println!(
            "缩略图生成 1024×768 → {}×{}: {elapsed:?}",
            g.width, g.height
        );
        let budget = if cfg!(debug_assertions) {
            std::time::Duration::from_millis(1000)
        } else {
            std::time::Duration::from_millis(100)
        };
        assert!(elapsed < budget, "缩略图生成 {elapsed:?} 超预算 {budget:?}");
    }
}
