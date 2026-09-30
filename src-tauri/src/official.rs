//! 官方表情包：安装包内置贴纸，首启/换库/版本升级时自动导入。
//!
//! Seam：`install_if_needed`。
//! - 幂等：图片按内容 hash 去重，重复导入自动跳过；
//! - 随库走：安装进度记录在库内 `app_meta` 表，换库/删库重建会重新导入；
//! - 可增量：`OFFICIAL_PACK_VERSION` +1 并补充 `OFFICIAL_STICKERS` 即可发布新包；
//!   升级时自动删除上一版贴纸（hash 清单见 `official_pack_hashes`）并清理旧名收藏夹；
//! - 失败不阻塞：调用方（setup）对错误只记日志，绝不影响应用启动。

use crate::library;
use crate::organize;
use rusqlite::params;
use sha2::{Digest, Sha256};
use std::path::Path;

/// 官方收藏夹名（sort_order = 0，恒排在所有用户收藏夹之前）。
pub const OFFICIAL_COLLECTION_NAME: &str = "青蛙 Mimu-01";

/// 旧版本用过的收藏夹名（升级安装时清理）。
const LEGACY_COLLECTION_NAMES: &[&str] = &["Mimu 官方表情包"];

/// 内置包版本：新增/修订贴纸时 +1，并在 `OFFICIAL_STICKERS` 里同步增改。
pub const OFFICIAL_PACK_VERSION: u32 = 2;

const META_KEY: &str = "official_pack_version";
const HASHES_KEY: &str = "official_pack_hashes";

pub struct OfficialSticker {
    /// 资源目录内的文件名（对应 `tauri.conf.json` 的 `bundle.resources`）。
    pub file: &'static str,
    pub description: &'static str,
    pub tags: &'static [&'static str],
}

/// 第一波青蛙 Mimu 官方贴纸（18 张）。
pub const OFFICIAL_STICKERS: &[OfficialSticker] = &[
    OfficialSticker {
        file: "mimu-a1-你好.png",
        description: "Mimu 开心地举起右手挥手打招呼，眨眼灿烂大笑，适合打招呼、问好、hi、开场破冰时使用",
        tags: &["Mimu", "你好", "打招呼", "问好", "hi"],
    },
    OfficialSticker {
        file: "mimu-a2-谢谢.png",
        description: "Mimu 双手捧着一颗大红心，幸福满满地说谢谢，适合表达感谢、感恩、多谢、比心",
        tags: &["Mimu", "谢谢", "感谢", "多谢", "爱心"],
    },
    OfficialSticker {
        file: "mimu-a3-好的OK.png",
        description: "Mimu 对镜头比出 OK 手势，干脆利落自信满满，适合回复好的、OK、收到、没问题、安排上",
        tags: &["Mimu", "好的", "OK", "收到", "没问题"],
    },
    OfficialSticker {
        file: "mimu-a4-在吗.png",
        description: "Mimu 从门边探出半个脑袋，好奇地问「在吗？」，适合找人、召唤好友、试探对方在不在",
        tags: &["Mimu", "在吗", "找人", "探头", "有人吗"],
    },
    OfficialSticker {
        file: "mimu-a5-大哭.png",
        description: "Mimu 嚎啕大哭，两道眼泪像瀑布一样喷涌，适合表达伤心、难过、泪崩、委屈巴巴",
        tags: &["Mimu", "大哭", "伤心", "泪崩", "呜呜"],
    },
    OfficialSticker {
        file: "mimu-a6-笑死.png",
        description: "Mimu 躺在地上打滚狂笑，笑出眼泪直不起腰，适合表达笑死、太好笑了、哈哈哈、乐坏了",
        tags: &["Mimu", "笑死", "大笑", "哈哈", "搞笑"],
    },
    OfficialSticker {
        file: "mimu-a7-哼.png",
        description: "Mimu 双手抱胸鼓起腮帮子，扭头傲娇地哼了一声，适合撒娇式生气、闹别扭、不理你",
        tags: &["Mimu", "哼", "傲娇", "生气", "不理你"],
    },
    OfficialSticker {
        file: "mimu-a8-怕怕.png",
        description: "Mimu 吓得缩进青蛙帽里，怯生生地发抖，适合表达害怕、恐惧、瑟瑟发抖、不敢看",
        tags: &["Mimu", "怕怕", "害怕", "瑟瑟发抖", "吓到"],
    },
    OfficialSticker {
        file: "mimu-c1-无语.png",
        description: "Mimu 面无表情死鱼眼，无语到生无可恋，适合表达无语、无奈、无话可说、流汗",
        tags: &["Mimu", "无语", "无奈", "无话可说", "流汗"],
    },
    OfficialSticker {
        file: "mimu-c2-摆烂.png",
        description: "Mimu 像冰淇淋一样融化瘫成一滩，彻底摆烂，适合表达躺平、不想动、放弃抵抗",
        tags: &["Mimu", "摆烂", "躺平", "不想动", "放弃"],
    },
    OfficialSticker {
        file: "mimu-c3-emo.png",
        description: "Mimu 抱膝蹲在下雨的乌云下，情绪低落，适合表达 emo、丧、难过、心情不好",
        tags: &["Mimu", "emo", "低落", "丧", "难过"],
    },
    OfficialSticker {
        file: "mimu-c4-裂开.png",
        description: "Mimu 像陶瓷一样从头顶裂到脚底，整个人碎成两半，适合表达裂开、心态崩了、崩溃、心碎",
        tags: &["Mimu", "裂开", "崩溃", "心碎", "碎了"],
    },
    OfficialSticker {
        file: "mimu-c5-绷不住了.png",
        description: "Mimu 捂着嘴拼命憋笑，嘴角疯狂上扬眼角带泪，适合表达绷不住、笑喷、忍不住、破功",
        tags: &["Mimu", "绷不住", "憋笑", "笑喷", "忍不住"],
    },
    OfficialSticker {
        file: "mimu-c6-摸鱼.png",
        description: "Mimu 摸着一条手绘小鱼，一脸惬意享受，适合表达摸鱼、偷懒、划水、带薪摸鱼",
        tags: &["Mimu", "摸鱼", "偷懒", "划水", "带薪摸鱼"],
    },
    OfficialSticker {
        file: "mimu-c7-拿捏.png",
        description: "Mimu 打出捏合手势，自信得意稳稳拿捏，适合表达拿捏、掌控、稳了、轻松搞定",
        tags: &["Mimu", "拿捏", "稳了", "掌控", "轻松搞定"],
    },
    OfficialSticker {
        file: "mimu-c8-灵魂出窍.png",
        description: "Mimu 呆坐放空，灵魂从头顶飘出，人在魂不在，适合表达灵魂出窍、发呆、走神、神游",
        tags: &["Mimu", "灵魂出窍", "放空", "发呆", "走神"],
    },
    OfficialSticker {
        file: "mimu-c9-满头问号.png",
        description: "Mimu 歪头一脸困惑，头顶飘满问号弹幕，适合表达问号、疑惑、什么情况、看不懂",
        tags: &["Mimu", "问号", "困惑", "疑惑", "什么情况"],
    },
    OfficialSticker {
        file: "mimu-c10-假笑营业.png",
        description: "Mimu 咧嘴露出职业假笑，身体前倾营业中，适合表达假笑、尬笑、营业、强颜欢笑",
        tags: &["Mimu", "假笑", "营业", "尬笑", "强颜欢笑"],
    },
];

#[derive(Debug, PartialEq, serde::Serialize)]
pub struct InstallSummary {
    pub imported: u32,
    pub skipped: u32,
    pub failed: u32,
}

fn installed_version(conn: &rusqlite::Connection) -> u32 {
    match library::get_meta(conn, META_KEY) {
        Ok(Some(v)) => v.parse().unwrap_or(0),
        _ => 0,
    }
}

/// 版本已最新 → `Ok(None)`；否则清理上一版、执行导入并推进版本记录。
pub fn install_if_needed(
    conn: &rusqlite::Connection,
    library_root: &Path,
    stickers_dir: &Path,
) -> Result<Option<InstallSummary>, String> {
    if installed_version(conn) >= OFFICIAL_PACK_VERSION {
        return Ok(None);
    }
    cleanup_previous_pack(conn, library_root);
    let (summary, hashes) = install(conn, library_root, stickers_dir)?;
    let manifest = serde_json::to_string(&hashes)
        .map_err(|e| format!("序列化官方包清单失败: {e}"))?;
    library::set_meta(conn, HASHES_KEY, &manifest)
        .map_err(|e| format!("记录官方包清单失败: {e}"))?;
    library::set_meta(conn, META_KEY, &OFFICIAL_PACK_VERSION.to_string())
        .map_err(|e| format!("记录官方包版本失败: {e}"))?;
    Ok(Some(summary))
}

/// 升级安装：删除上一版贴纸（含库内文件与缩略图）和旧版本名收藏夹。
/// 首次安装无上一版清单，此函数为空操作。
fn cleanup_previous_pack(conn: &rusqlite::Connection, library_root: &Path) {
    if let Ok(Some(json)) = library::get_meta(conn, HASHES_KEY) {
        if let Ok(hashes) = serde_json::from_str::<Vec<String>>(&json) {
            for hash in hashes {
                match library::find_meme_id(conn, &hash) {
                    Ok(Some(meme_id)) => {
                        if let Err(e) = organize::delete_meme(conn, library_root, meme_id) {
                            eprintln!("清理上一版官方贴纸失败 {hash}: {e}");
                        }
                    }
                    Ok(None) => {}
                    Err(e) => eprintln!("查询上一版官方贴纸失败 {hash}: {e}"),
                }
            }
        }
    }
    for name in LEGACY_COLLECTION_NAMES {
        if let Ok(id) = conn.query_row(
            "SELECT id FROM collection WHERE name = ?1",
            params![name],
            |r| r.get::<_, i64>(0),
        ) {
            let _ = organize::delete_collection(conn, id);
        }
    }
}

/// 官方收藏夹：同名已存在则复用（不干预用户可能调整过的排序）；
/// 新建时固定 sort_order = 0，恒排最前。
fn ensure_official_collection(conn: &rusqlite::Connection) -> Result<i64, String> {
    if let Ok(id) = conn.query_row(
        "SELECT id FROM collection WHERE name = ?1",
        params![OFFICIAL_COLLECTION_NAME],
        |r| r.get::<_, i64>(0),
    ) {
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO collection (name, sort_order, created_at) VALUES (?1, 0, strftime('%s','now'))",
        params![OFFICIAL_COLLECTION_NAME],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

fn install(
    conn: &rusqlite::Connection,
    library_root: &Path,
    stickers_dir: &Path,
) -> Result<(InstallSummary, Vec<String>), String> {
    let collection_id = ensure_official_collection(conn)?;
    let mut summary = InstallSummary { imported: 0, skipped: 0, failed: 0 };
    let mut hashes: Vec<String> = Vec::new();
    for sticker in OFFICIAL_STICKERS {
        let path = stickers_dir.join(sticker.file);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => {
                summary.failed += 1;
                eprintln!("官方贴纸读取失败: {}", sticker.file);
                continue;
            }
        };
        let mut result = library::ImportResult {
            imported: 0,
            skipped: 0,
            unsupported: 0,
            failed: 0,
            content_hashes: Vec::new(),
        };
        library::import_file(conn, library_root, &path, Some(collection_id), &mut result);
        summary.imported += result.imported;
        summary.skipped += result.skipped;
        if result.failed > 0 || result.unsupported > 0 {
            summary.failed += 1;
            eprintln!("官方贴纸导入失败: {}", sticker.file);
            continue;
        }
        // import_file 的 skipped 分支不返回 hash，这里按文件内容自行定位目标 meme。
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let hash = format!("{:x}", hasher.finalize());
        match library::find_meme_id(conn, &hash) {
            Ok(Some(meme_id)) => {
                hashes.push(hash);
                if result.imported > 0 {
                    if let Err(e) = organize::set_description(conn, meme_id, sticker.description)
                    {
                        eprintln!("官方贴纸描述设置失败 {}: {e}", sticker.file);
                    }
                    for tag in sticker.tags {
                        if let Err(e) = organize::add_tag(conn, meme_id, tag) {
                            eprintln!("官方贴纸标签设置失败 {}: {e}", sticker.file);
                        }
                    }
                } else {
                    // 库里已有同内容图（旧库/重复导入）：收编进官方收藏夹；
                    // 描述为空才补，尊重用户自定义。
                    if let Err(e) =
                        organize::add_meme_to_collection(conn, meme_id, collection_id)
                    {
                        eprintln!("官方贴纸收编失败 {}: {e}", sticker.file);
                    }
                    let desc: Option<String> = conn
                        .query_row(
                            "SELECT description FROM meme WHERE id = ?1",
                            params![meme_id],
                            |r| r.get(0),
                        )
                        .ok();
                    if desc.as_deref().unwrap_or("").is_empty() {
                        let _ = organize::set_description(conn, meme_id, sticker.description);
                    }
                }
            }
            Ok(None) => {
                summary.failed += 1;
                eprintln!("官方贴纸入库后未找到记录: {}", sticker.file);
            }
            Err(e) => {
                summary.failed += 1;
                eprintln!("查询官方贴纸失败 {}: {e}", sticker.file);
            }
        }
    }
    Ok((summary, hashes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn png_bytes(seed: u8) -> Vec<u8> {
        let img: image::RgbaImage = image::ImageBuffer::from_fn(4, 4, |x, y| {
            image::Rgba([seed, (x * 7 + y * 13) as u8, seed ^ ((x + y) as u8), 255])
        });
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    struct Env {
        _tmp: tempfile::TempDir,
        conn: rusqlite::Connection,
        lib: PathBuf,
        stickers: PathBuf,
    }

    fn setup(with_files: bool) -> Env {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let conn = library::open_db(&lib).unwrap();
        let stickers = tmp.path().join("resources").join("official");
        std::fs::create_dir_all(&stickers).unwrap();
        if with_files {
            for (i, s) in OFFICIAL_STICKERS.iter().enumerate() {
                std::fs::write(stickers.join(s.file), png_bytes((i % 251) as u8)).unwrap();
            }
        }
        Env { _tmp: tmp, conn, lib, stickers }
    }

    #[test]
    fn first_run_imports_all_stickers_with_metadata() {
        let e = setup(true);
        let s = install_if_needed(&e.conn, &e.lib, &e.stickers).unwrap().unwrap();
        assert_eq!(s.imported, OFFICIAL_STICKERS.len() as u32);
        assert_eq!(s.skipped, 0);
        assert_eq!(s.failed, 0);

        let cols = library::list_collections(&e.conn).unwrap();
        let col = cols.iter().find(|c| c.name == OFFICIAL_COLLECTION_NAME).expect("官方收藏夹存在");
        assert_eq!(col.sort_order, 0, "官方收藏夹排在最前");

        let memes =
            library::list_memes(&e.conn, &library::GalleryView::Collection(col.id)).unwrap();
        assert_eq!(memes.len(), OFFICIAL_STICKERS.len());
        for m in &memes {
            assert!(m.description.len() >= 20, "描述足够详细");
            assert_eq!(m.tags.len(), 5, "每张贴纸 5 个标签");
            assert!(m.tags.contains(&"Mimu".to_string()));
        }
        assert_eq!(installed_version(&e.conn), OFFICIAL_PACK_VERSION);
    }

    #[test]
    fn second_run_is_noop() {
        let e = setup(true);
        install_if_needed(&e.conn, &e.lib, &e.stickers).unwrap().unwrap();
        assert!(install_if_needed(&e.conn, &e.lib, &e.stickers).unwrap().is_none());
        assert_eq!(
            library::list_memes(&e.conn, &library::GalleryView::All).unwrap().len(),
            OFFICIAL_STICKERS.len()
        );
    }

    #[test]
    fn missing_sticker_files_are_tolerated() {
        let e = setup(false);
        let s = install_if_needed(&e.conn, &e.lib, &e.stickers).unwrap().unwrap();
        assert_eq!(s.imported, 0);
        assert_eq!(s.failed, OFFICIAL_STICKERS.len() as u32);
        // 版本照常推进，不会每次启动反复重试
        assert!(install_if_needed(&e.conn, &e.lib, &e.stickers).unwrap().is_none());
    }

    #[test]
    fn existing_same_content_meme_is_adopted_into_official_collection() {
        let e = setup(true);
        // 模拟旧库：用户手动导入过同内容图（先取走第一个文件再安装）
        let src = e.stickers.join(OFFICIAL_STICKERS[0].file);
        let bytes = std::fs::read(&src).unwrap();
        let side = e._tmp.path().join("side").join("my-copy.png");
        std::fs::create_dir_all(side.parent().unwrap()).unwrap();
        std::fs::write(&side, bytes).unwrap();
        library::import_paths(&e.conn, &e.lib, &[side]).unwrap();

        let s = install_if_needed(&e.conn, &e.lib, &e.stickers).unwrap().unwrap();
        assert_eq!(s.imported, OFFICIAL_STICKERS.len() as u32 - 1);
        assert_eq!(s.skipped, 1);

        let cols = library::list_collections(&e.conn).unwrap();
        let col = cols.iter().find(|c| c.name == OFFICIAL_COLLECTION_NAME).unwrap();
        let in_col =
            library::list_memes(&e.conn, &library::GalleryView::Collection(col.id)).unwrap();
        assert_eq!(in_col.len(), OFFICIAL_STICKERS.len(), "已有同内容图被收编");
    }

    #[test]
    fn version_upgrade_replaces_previous_pack_and_legacy_collection() {
        let e = setup(true);
        install_if_needed(&e.conn, &e.lib, &e.stickers).unwrap().unwrap();

        // 模拟「上一版」状态：版本回拨、当前贴纸登记为上一版清单、旧名收藏夹残留
        let hashes: Vec<String> = OFFICIAL_STICKERS
            .iter()
            .map(|s| sha256_hex(&std::fs::read(e.stickers.join(s.file)).unwrap()))
            .collect();
        library::set_meta(&e.conn, "official_pack_version", "1").unwrap();
        library::set_meta(&e.conn, HASHES_KEY, &serde_json::to_string(&hashes).unwrap()).unwrap();
        e.conn
            .execute(
                "INSERT INTO collection (name, sort_order, created_at) VALUES ('Mimu 官方表情包', 0, 0)",
                [],
            )
            .unwrap();

        let s = install_if_needed(&e.conn, &e.lib, &e.stickers).unwrap().unwrap();
        assert_eq!(s.imported, OFFICIAL_STICKERS.len() as u32, "旧贴纸被删除后重新导入");
        assert_eq!(
            library::list_memes(&e.conn, &library::GalleryView::All).unwrap().len(),
            OFFICIAL_STICKERS.len(),
            "总数不翻倍"
        );
        let cols = library::list_collections(&e.conn).unwrap();
        assert!(cols.iter().all(|c| c.name != "Mimu 官方表情包"), "旧名收藏夹被清理");
        assert!(cols.iter().any(|c| c.name == OFFICIAL_COLLECTION_NAME));
        assert_eq!(installed_version(&e.conn), OFFICIAL_PACK_VERSION);
    }
}
