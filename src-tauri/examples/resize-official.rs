//! 一次性工具：官方贴纸压缩到约 0.88MB 并改为中文文件名。
//! 用法：cargo run --release --bin resize-official（在 src-tauri 目录下执行）。

use std::path::PathBuf;

const DIR: &str = "resources/official";
const MAX_BYTES: usize = 920 * 1024;
const MIN_WIDTH: u32 = 512;

fn main() {
    let renames: &[(&str, &str)] = &[
        ("mimu-a1-hello.png", "mimu-a1-你好.png"),
        ("mimu-a2-thanks.png", "mimu-a2-谢谢.png"),
        ("mimu-a3-ok.png", "mimu-a3-好的OK.png"),
        ("mimu-a4-you-there.png", "mimu-a4-在吗.png"),
        ("mimu-a5-crying.png", "mimu-a5-大哭.png"),
        ("mimu-a6-rofl.png", "mimu-a6-笑死.png"),
        ("mimu-a7-hmph.png", "mimu-a7-哼.png"),
        ("mimu-a8-scared.png", "mimu-a8-怕怕.png"),
        ("mimu-c1-speechless.png", "mimu-c1-无语.png"),
        ("mimu-c2-melted.png", "mimu-c2-摆烂.png"),
        ("mimu-c3-emo.png", "mimu-c3-emo.png"),
        ("mimu-c4-cracked.png", "mimu-c4-裂开.png"),
        ("mimu-c5-losing-it.png", "mimu-c5-绷不住了.png"),
        ("mimu-c6-slacking.png", "mimu-c6-摸鱼.png"),
        ("mimu-c7-in-control.png", "mimu-c7-拿捏.png"),
        ("mimu-c8-soul-out.png", "mimu-c8-灵魂出窍.png"),
        ("mimu-c9-question-mark.png", "mimu-c9-满头问号.png"),
        ("mimu-c10-fake-smile.png", "mimu-c10-假笑营业.png"),
    ];
    let dir = PathBuf::from(DIR);
    for (old, new) in renames {
        let src = dir.join(old);
        let img = image::open(&src).unwrap_or_else(|e| panic!("打开失败 {old}: {e}"));
        let rgba = img.to_rgba8();
        let (ow, oh) = (rgba.width(), rgba.height());
        let mut lo = MIN_WIDTH.min(ow);
        let mut hi = ow;
        let mut best: Option<(u32, u32, Vec<u8>)> = None;
        while lo <= hi {
            let mid = (lo + hi) / 2;
            let h = (oh as f64 * mid as f64 / ow as f64).round() as u32;
            let resized = image::imageops::resize(
                &rgba,
                mid,
                h,
                image::imageops::FilterType::CatmullRom,
            );
            let mut buf = Vec::new();
            image::DynamicImage::ImageRgba8(resized)
                .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
                .unwrap();
            if buf.len() <= MAX_BYTES {
                best = Some((mid, h, buf));
                lo = mid + 1;
            } else if mid == 0 {
                break;
            } else {
                hi = mid - 1;
            }
        }
        let (w, h, bytes) = best.expect("512px 仍超限，需人工处理");
        let dst = dir.join(new);
        std::fs::write(&dst, &bytes).unwrap();
        if old != new {
            std::fs::remove_file(&src).unwrap();
        }
        println!("{new}: {w}x{h}px, {:.0} KB", bytes.len() as f64 / 1024.0);
    }
}
