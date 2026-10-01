//! Managed Library: 导入、内容去重、目录登记。
//!
//! Seam：`open_db` / `import_paths` / `list_memes` / `list_collections`。

use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, serde::Serialize)]
pub struct ImportResult {
    pub imported: u32,
    pub skipped: u32,
    pub unsupported: u32,
    pub failed: u32,
    /// 本批成功导入的 content hash（供缩略图管道顺带生成）
    pub content_hashes: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct Meme {
    pub id: i64,
    pub internal_path: String,
    pub original_filename: String,
    pub extension: String,
    pub mime_type: String,
    pub width: i64,
    pub height: i64,
    pub file_size: i64,
    pub content_hash: String,
    pub description: String,
    pub is_favorite: bool,
    pub last_used_at: Option<i64>,
    /// 缓存中已有缩略图时为绝对路径（由 commands 层填充），否则 None
    pub thumbnail_path: Option<String>,
    /// 该 Meme 的全部标签名（关系表聚合）
    pub tags: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub sort_order: i64,
    pub group_id: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct CollectionGroup {
    pub id: i64,
    pub name: String,
    pub sort_order: i64,
}

const SCHEMA_VERSION: i64 = 2;

pub fn open_db(library_root: &Path) -> rusqlite::Result<Connection> {
    std::fs::create_dir_all(library_root).map_err(|_| {
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CANTOPEN),
            Some("无法创建表情库目录".into()),
        )
    })?;
    let conn = Connection::open(library_root.join("library.db"))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS meme (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            internal_path TEXT NOT NULL UNIQUE,
            original_filename TEXT NOT NULL,
            extension TEXT NOT NULL,
            mime_type TEXT NOT NULL,
            width INTEGER NOT NULL DEFAULT 0,
            height INTEGER NOT NULL DEFAULT 0,
            file_size INTEGER NOT NULL DEFAULT 0,
            content_hash TEXT NOT NULL UNIQUE,
            description TEXT NOT NULL DEFAULT '',
            is_favorite INTEGER NOT NULL DEFAULT 0,
            imported_at INTEGER NOT NULL,
            last_used_at INTEGER
        );
        CREATE TABLE IF NOT EXISTS tag (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            normalized_name TEXT NOT NULL UNIQUE,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS collection (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS meme_tag (
            meme_id INTEGER NOT NULL REFERENCES meme(id) ON DELETE CASCADE,
            tag_id INTEGER NOT NULL REFERENCES tag(id) ON DELETE CASCADE,
            PRIMARY KEY (meme_id, tag_id)
        );
        CREATE TABLE IF NOT EXISTS meme_collection (
            meme_id INTEGER NOT NULL REFERENCES meme(id) ON DELETE CASCADE,
            collection_id INTEGER NOT NULL REFERENCES collection(id) ON DELETE CASCADE,
            PRIMARY KEY (meme_id, collection_id)
        );
        CREATE TABLE IF NOT EXISTS app_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version < SCHEMA_VERSION {
        conn.execute_batch(
            "BEGIN;
             CREATE TABLE collection_group (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT NOT NULL UNIQUE,
                 sort_order INTEGER NOT NULL DEFAULT 0,
                 created_at INTEGER NOT NULL
             );
             ALTER TABLE collection ADD COLUMN group_id INTEGER REFERENCES collection_group(id) ON DELETE SET NULL;
             CREATE INDEX idx_collection_group_order ON collection(group_id, sort_order, id);
             PRAGMA user_version = 2;
             COMMIT;",
        )?;
    }
    Ok(())
}

const SUPPORTED_EXTENSIONS: [&str; 7] = ["png", "jpg", "jpeg", "webp", "gif", "bmp", "apng"];

pub fn import_paths(
    conn: &Connection,
    library_root: &Path,
    paths: &[PathBuf],
) -> rusqlite::Result<ImportResult> {
    import_paths_into(conn, library_root, paths, None)
}

pub fn import_paths_into(
    conn: &Connection,
    library_root: &Path,
    paths: &[PathBuf],
    target_collection_id: Option<i64>,
) -> rusqlite::Result<ImportResult> {
    if let Some(cid) = target_collection_id {
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM collection WHERE id = ?1)",
            params![cid],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
    }
    let mut result = ImportResult {
        imported: 0,
        skipped: 0,
        unsupported: 0,
        failed: 0,
        content_hashes: Vec::new(),
    };
    for path in paths {
        if path.is_dir() {
            import_directory(conn, library_root, path, target_collection_id, &mut result)?;
        } else {
            import_file_to(
                conn,
                library_root,
                path,
                None,
                target_collection_id,
                &mut result,
            );
        }
    }
    Ok(result)
}

fn import_directory(
    conn: &Connection,
    library_root: &Path,
    dir: &Path,
    target_collection_id: Option<i64>,
    result: &mut ImportResult,
) -> rusqlite::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => {
            result.failed += 1;
            return Ok(());
        }
    };
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            subdirs.push(p);
        }
    }
    // 一级子文件夹 → 收藏夹（递归扫描其中的图片）
    for sub in subdirs {
        let name = sub
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let collection_id = ensure_collection(conn, &name)?;
        let files = collect_files(&sub);
        for file in files {
            import_file_to(
                conn,
                library_root,
                &file,
                Some(collection_id),
                target_collection_id,
                result,
            );
        }
    }
    // 目录直属文件（不含子目录内容）
    let files = collect_files(dir);
    for file in files {
        import_file_to(
            conn,
            library_root,
            &file,
            None,
            target_collection_id,
            result,
        );
    }
    Ok(())
}

fn collect_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return files;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() {
            files.push(p);
        }
    }
    files
}

fn ensure_collection(conn: &Connection, name: &str) -> rusqlite::Result<i64> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM collection WHERE name = ?1",
            params![name],
            |r| r.get(0),
        )
        .ok();
    if let Some(id) = existing {
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO collection (name, sort_order, created_at) VALUES (?1, (SELECT COALESCE(MAX(sort_order),0)+1 FROM collection), strftime('%s','now'))",
        params![name],
    )?;
    Ok(conn.last_insert_rowid())
}

pub(crate) fn import_file(
    conn: &Connection,
    library_root: &Path,
    path: &Path,
    collection_id: Option<i64>,
    result: &mut ImportResult,
) {
    import_file_to(conn, library_root, path, collection_id, None, result);
}

fn import_file_to(
    conn: &Connection,
    library_root: &Path,
    path: &Path,
    collection_id: Option<i64>,
    target_collection_id: Option<i64>,
    result: &mut ImportResult,
) {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if !SUPPORTED_EXTENSIONS.contains(&ext.as_str()) {
        result.unsupported += 1;
        return;
    }
    let outcome = (|| -> Result<String, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(path)?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let hash = format!("{:x}", hasher.finalize());
        let existing_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM meme WHERE content_hash = ?1",
                params![&hash],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(meme_id) = existing_id {
            attach_import_memberships(conn, meme_id, collection_id, target_collection_id)?;
            return Err(DupError.into());
        }
        let reader = image::ImageReader::new(Cursor::new(&bytes))
            .with_guessed_format()
            .map_err(std::io::Error::other)?;
        let format = reader
            .format()
            .ok_or(std::io::Error::other("未知图片格式"))?;
        let (width, height) = reader.into_dimensions().map_err(std::io::Error::other)?;
        let mime = match format {
            image::ImageFormat::Png => "image/png",
            image::ImageFormat::Jpeg => "image/jpeg",
            image::ImageFormat::Gif => "image/gif",
            image::ImageFormat::WebP => "image/webp",
            image::ImageFormat::Bmp => "image/bmp",
            _ => "application/octet-stream",
        };
        let internal_name = format!("{hash}.{ext}");
        let internal_path = library_root.join(&internal_name);
        std::fs::write(&internal_path, &bytes)?;
        let original_filename = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO meme (internal_path, original_filename, extension, mime_type, width, height, file_size, content_hash, imported_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                internal_name,
                original_filename,
                ext,
                mime,
                width as i64,
                height as i64,
                bytes.len() as i64,
                hash,
                now
            ],
        )?;
        let meme_id = conn.last_insert_rowid();
        attach_import_memberships(conn, meme_id, collection_id, target_collection_id)?;
        Ok(hash)
    })();
    match outcome {
        Ok(hash) => {
            result.imported += 1;
            result.content_hashes.push(hash);
        }
        Err(e) if e.is::<DupError>() => result.skipped += 1,
        Err(_) => result.failed += 1,
    }
}

fn attach_import_memberships(
    conn: &Connection,
    meme_id: i64,
    folder_id: Option<i64>,
    target_id: Option<i64>,
) -> rusqlite::Result<()> {
    for cid in [folder_id, target_id].into_iter().flatten() {
        conn.execute(
            "INSERT OR IGNORE INTO meme_collection (meme_id, collection_id) VALUES (?1, ?2)",
            params![meme_id, cid],
        )?;
    }
    Ok(())
}

struct DupError;
impl std::fmt::Debug for DupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("duplicate")
    }
}
impl std::fmt::Display for DupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("duplicate content")
    }
}
impl std::error::Error for DupError {}

/// 图库视图：系统三个固定视图 + 用户收藏夹（邻接标签 serde，前端传 `{kind, id?}`）。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum GalleryView {
    All,
    Favorites,
    Recent,
    Collection(i64),
    Collections(Vec<i64>),
}

const MEME_SELECT: &str = "SELECT m.id, m.internal_path, m.original_filename, m.extension, m.mime_type, m.width, m.height, m.file_size, m.content_hash, m.description, m.is_favorite, m.last_used_at,
    (SELECT GROUP_CONCAT(t.name, char(31)) FROM meme_tag mt JOIN tag t ON t.id = mt.tag_id WHERE mt.meme_id = m.id)
    FROM meme m";

fn map_meme_row(r: &rusqlite::Row) -> rusqlite::Result<Meme> {
    let tags_csv: Option<String> = r.get(12)?;
    Ok(Meme {
        id: r.get(0)?,
        internal_path: r.get(1)?,
        original_filename: r.get(2)?,
        extension: r.get(3)?,
        mime_type: r.get(4)?,
        width: r.get(5)?,
        height: r.get(6)?,
        file_size: r.get(7)?,
        content_hash: r.get(8)?,
        description: r.get(9)?,
        is_favorite: r.get::<_, i64>(10)? != 0,
        last_used_at: r.get(11)?,
        thumbnail_path: None,
        tags: tags_csv
            .map(|s| s.split('\u{1f}').map(str::to_string).collect())
            .unwrap_or_default(),
    })
}

/// 按视图列图。最近使用：只含有 last_used_at 的，按最近复制倒序（列即去重，
/// 因为 last_used_at 是 Meme 上单值列，同一条图只保留最新一次复制时间）。
pub fn list_memes(conn: &Connection, view: &GalleryView) -> rusqlite::Result<Vec<Meme>> {
    let rows = match view {
        GalleryView::All => {
            let sql = format!("{MEME_SELECT} ORDER BY m.id");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map([], map_meme_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        }
        GalleryView::Favorites => {
            let sql = format!("{MEME_SELECT} WHERE m.is_favorite = 1 ORDER BY m.id");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map([], map_meme_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        }
        GalleryView::Recent => {
            let sql = format!(
                "{MEME_SELECT} WHERE m.last_used_at IS NOT NULL ORDER BY m.last_used_at DESC"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map([], map_meme_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        }
        GalleryView::Collection(cid) => {
            let sql = format!(
                "{MEME_SELECT} JOIN meme_collection mc ON mc.meme_id = m.id
                 WHERE mc.collection_id = ?1 ORDER BY m.id"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(params![cid], map_meme_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        }
        GalleryView::Collections(ids) => {
            if ids.is_empty() {
                return Ok(Vec::new());
            }
            let placeholders = (1..=ids.len())
                .map(|i| format!("?{i}"))
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "{MEME_SELECT} WHERE EXISTS (SELECT 1 FROM meme_collection mc WHERE mc.meme_id = m.id AND mc.collection_id IN ({placeholders})) ORDER BY m.id"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt
                .query_map(rusqlite::params_from_iter(ids.iter()), map_meme_row)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        }
    };
    Ok(rows)
}

/// LIKE 通配符转义：用户输入一律按字面量子串匹配。
fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// 搜索：LIKE 子串起步（spec 决策，FTS5 留作演进）。
/// - 一次查询同时命中文件名 / 标签 / 描述
/// - 多关键词空白分隔，AND 语义
/// - ASCII 大小写不敏感（SQLite LIKE 默认），中文按码点精确子串匹配
/// - 范围限定：在传入的视图（收藏夹/收藏/最近使用）内搜索；空查询等价于列全视图
pub fn search_memes(
    conn: &Connection,
    view: &GalleryView,
    query: &str,
) -> rusqlite::Result<Vec<Meme>> {
    let keywords: Vec<&str> = query.split_whitespace().collect();
    if keywords.is_empty() {
        return list_memes(conn, view);
    }

    let mut join = String::new();
    let mut clauses: Vec<String> = Vec::new();
    let mut args: Vec<String> = Vec::new();
    let order = match view {
        GalleryView::All => "m.id",
        GalleryView::Favorites => {
            clauses.push("m.is_favorite = 1".into());
            "m.id"
        }
        GalleryView::Recent => {
            clauses.push("m.last_used_at IS NOT NULL".into());
            "m.last_used_at DESC"
        }
        GalleryView::Collection(cid) => {
            join = " JOIN meme_collection mc ON mc.meme_id = m.id".into();
            clauses.push(format!("mc.collection_id = ?{}", args.len() + 1));
            args.push(cid.to_string());
            "m.id"
        }
        GalleryView::Collections(ids) => {
            if ids.is_empty() {
                return Ok(Vec::new());
            }
            let placeholders = ids
                .iter()
                .map(|id| {
                    args.push(id.to_string());
                    format!("?{}", args.len())
                })
                .collect::<Vec<_>>()
                .join(",");
            clauses.push(format!(
                "EXISTS (SELECT 1 FROM meme_collection mc WHERE mc.meme_id = m.id AND mc.collection_id IN ({placeholders}))"
            ));
            "m.id"
        }
    };
    for keyword in keywords {
        let pattern = format!("%{}%", escape_like(keyword));
        let (p_name, p_desc, p_tag) = (args.len() + 1, args.len() + 2, args.len() + 3);
        args.push(pattern.clone());
        args.push(pattern.clone());
        args.push(pattern);
        clauses.push(format!(
            "(m.original_filename LIKE ?{p_name} ESCAPE '\\' \
             OR m.description LIKE ?{p_desc} ESCAPE '\\' \
             OR EXISTS (SELECT 1 FROM meme_tag mt JOIN tag t ON t.id = mt.tag_id \
                        WHERE mt.meme_id = m.id AND t.name LIKE ?{p_tag} ESCAPE '\\'))"
        ));
    }

    let sql = format!(
        "{MEME_SELECT}{join} WHERE {} ORDER BY {order}",
        clauses.join(" AND ")
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(args.iter()), map_meme_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// 按 content hash 找库内文件相对路径（同内容去重保证最多一条）。
pub fn find_internal_path(
    conn: &Connection,
    content_hash: &str,
) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT internal_path FROM meme WHERE content_hash = ?1",
        params![content_hash],
        |r| r.get(0),
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(other),
    })
}

/// 按 id 取表情的库内文件信息：（相对路径、扩展名、content hash、显示文件名）。
pub fn find_meme_file(
    conn: &Connection,
    meme_id: i64,
) -> rusqlite::Result<Option<(String, String, String, String)>> {
    conn.query_row(
        "SELECT internal_path, extension, content_hash, original_filename FROM meme WHERE id = ?1",
        params![meme_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(other),
    })
}

pub fn list_collections(conn: &Connection) -> rusqlite::Result<Vec<Collection>> {
    let mut stmt = conn
        .prepare("SELECT id, name, sort_order, group_id FROM collection ORDER BY sort_order, id")?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Collection {
                id: r.get(0)?,
                name: r.get(1)?,
                sort_order: r.get(2)?,
                group_id: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn list_collection_groups(conn: &Connection) -> rusqlite::Result<Vec<CollectionGroup>> {
    let mut stmt =
        conn.prepare("SELECT id, name, sort_order FROM collection_group ORDER BY sort_order, id")?;
    let groups = stmt
        .query_map([], |r| {
            Ok(CollectionGroup {
                id: r.get(0)?,
                name: r.get(1)?,
                sort_order: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(groups)
}

/// 读库内元数据（官方包版本等随库状态；换库/删库后自然回到未安装态）。
pub fn get_meta(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM app_meta WHERE key = ?1",
        params![key],
        |r| r.get(0),
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(other),
    })
}

/// 写库内元数据（UPSERT）。
pub fn set_meta(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO app_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// 按 content hash 查 meme id（官方包导入后设置元数据用）。
pub fn find_meme_id(conn: &Connection, content_hash: &str) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT id FROM meme WHERE content_hash = ?1",
        params![content_hash],
        |r| r.get(0),
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(other),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, RgbImage};
    use std::fs;

    #[test]
    fn migration_preserves_v1_collection_ids_and_membership() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("library.db");
        let old = Connection::open(&path).unwrap();
        old.execute_batch(
            "CREATE TABLE collection (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE, sort_order INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL);
             CREATE TABLE meme (id INTEGER PRIMARY KEY AUTOINCREMENT, internal_path TEXT NOT NULL UNIQUE, original_filename TEXT NOT NULL, extension TEXT NOT NULL, mime_type TEXT NOT NULL, width INTEGER NOT NULL DEFAULT 0, height INTEGER NOT NULL DEFAULT 0, file_size INTEGER NOT NULL DEFAULT 0, content_hash TEXT NOT NULL UNIQUE, description TEXT NOT NULL DEFAULT '', is_favorite INTEGER NOT NULL DEFAULT 0, imported_at INTEGER NOT NULL, last_used_at INTEGER);
             CREATE TABLE meme_collection (meme_id INTEGER NOT NULL REFERENCES meme(id) ON DELETE CASCADE, collection_id INTEGER NOT NULL REFERENCES collection(id) ON DELETE CASCADE, PRIMARY KEY (meme_id, collection_id));
             INSERT INTO collection VALUES (42, '旧收藏夹', 7, 0);
             INSERT INTO meme (id,internal_path,original_filename,extension,mime_type,content_hash,imported_at) VALUES (9,'9.png','9.png','png','image/png','oldhash',0);
             INSERT INTO meme_collection VALUES (9,42);
             PRAGMA user_version = 1;",
        ).unwrap();
        drop(old);

        let upgraded = open_db(tmp.path()).unwrap();
        let collection = &list_collections(&upgraded).unwrap()[0];
        assert_eq!((collection.id, collection.group_id), (42, None));
        assert_eq!(
            list_memes(&upgraded, &GalleryView::Collection(42))
                .unwrap()
                .len(),
            1
        );
        assert!(list_collection_groups(&upgraded).unwrap().is_empty());
        assert_eq!(
            upgraded
                .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            2
        );
    }

    fn png_bytes(seed: u8) -> Vec<u8> {
        let img: RgbImage = ImageBuffer::from_fn(3, 3, |x, y| {
            image::Rgb([(x * 40 + seed as u32) as u8, (y * 40) as u8, 128])
        });
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let p = dir.join(name);
        fs::write(&p, bytes).unwrap();
        p
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    #[test]
    fn imports_a_single_image_into_managed_library() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let conn = open_db(&lib).unwrap();
        let src = write_file(tmp.path(), "cat.png", &png_bytes(1));

        let r = import_paths(&conn, &lib, &[src]).unwrap();

        assert_eq!(
            r,
            ImportResult {
                imported: 1,
                skipped: 0,
                unsupported: 0,
                failed: 0,
                content_hashes: vec![list_memes(&conn, &GalleryView::All).unwrap()[0]
                    .content_hash
                    .clone()]
            }
        );
        let files: Vec<_> = fs::read_dir(&lib).unwrap().flatten().collect();
        assert!(files
            .iter()
            .any(|f| f.path().extension().is_some_and(|e| e == "png")));
        let memes = list_memes(&conn, &GalleryView::All).unwrap();
        assert_eq!(memes.len(), 1);
        assert_eq!(memes[0].original_filename, "cat.png");
    }

    #[test]
    fn skips_exact_duplicate_content() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let conn = open_db(&lib).unwrap();
        let a = write_file(tmp.path(), "a.png", &png_bytes(1));
        let b = write_file(tmp.path(), "b.png", &png_bytes(1)); // 同内容不同名

        import_paths(&conn, &lib, &[a]).unwrap();
        let r = import_paths(&conn, &lib, &[b]).unwrap();

        assert_eq!(
            r,
            ImportResult {
                imported: 0,
                skipped: 1,
                unsupported: 0,
                failed: 0,
                content_hashes: Vec::new()
            }
        );
        assert_eq!(list_memes(&conn, &GalleryView::All).unwrap().len(), 1);
    }

    #[test]
    fn import_into_current_collection_attaches_new_and_existing_images() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let conn = open_db(&lib).unwrap();
        conn.execute(
            "INSERT INTO collection (name, created_at) VALUES ('目标', 0)",
            [],
        )
        .unwrap();
        let target = conn.last_insert_rowid();
        let existing = write_file(tmp.path(), "existing.png", &png_bytes(11));
        let fresh = write_file(tmp.path(), "fresh.png", &png_bytes(12));
        import_paths(&conn, &lib, &[existing.clone()]).unwrap();
        let result = import_paths_into(&conn, &lib, &[existing, fresh], Some(target)).unwrap();
        assert_eq!((result.imported, result.skipped, result.failed), (1, 1, 0));
        assert_eq!(
            list_memes(&conn, &GalleryView::Collection(target))
                .unwrap()
                .len(),
            2
        );
        assert_eq!(list_memes(&conn, &GalleryView::All).unwrap().len(), 2);
        let folder = tmp.path().join("folder");
        fs::create_dir_all(folder.join("子目录")).unwrap();
        write_file(&folder, "root.png", &png_bytes(13));
        write_file(&folder.join("子目录"), "child.png", &png_bytes(14));
        let folder_result = import_paths_into(&conn, &lib, &[folder], Some(target)).unwrap();
        assert_eq!(folder_result.imported, 2);
        assert_eq!(
            list_memes(&conn, &GalleryView::Collection(target))
                .unwrap()
                .len(),
            4
        );
        let child = list_collections(&conn)
            .unwrap()
            .into_iter()
            .find(|c| c.name == "子目录")
            .unwrap();
        assert_eq!(
            list_memes(&conn, &GalleryView::Collection(child.id))
                .unwrap()
                .len(),
            1
        );
        assert!(import_paths_into(&conn, &lib, &[], Some(target + 999)).is_err());
    }

    #[test]
    fn multiple_collection_search_uses_union_without_duplicates() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let conn = open_db(&lib).unwrap();
        conn.execute(
            "INSERT INTO collection (name, created_at) VALUES ('一', 0), ('二', 0)",
            [],
        )
        .unwrap();
        let first = list_collections(&conn).unwrap()[0].id;
        let second = list_collections(&conn).unwrap()[1].id;
        let a = write_file(tmp.path(), "cat.png", &png_bytes(21));
        let b = write_file(tmp.path(), "dog.png", &png_bytes(22));
        import_paths_into(&conn, &lib, &[a.clone()], Some(first)).unwrap();
        import_paths_into(&conn, &lib, &[a, b], Some(second)).unwrap();
        let view = GalleryView::Collections(vec![first, second]);
        assert_eq!(list_memes(&conn, &view).unwrap().len(), 2);
        assert_eq!(search_memes(&conn, &view, "cat").unwrap().len(), 1);
        assert!(list_memes(&conn, &GalleryView::Collections(vec![]))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn folder_import_creates_collections_from_first_level_subfolders() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let conn = open_db(&lib).unwrap();
        let root = tmp.path().join("表情包");
        fs::create_dir_all(root.join("猫猫")).unwrap();
        fs::create_dir_all(root.join("工作")).unwrap();
        write_file(&root.join("猫猫"), "a.png", &png_bytes(1));
        write_file(&root.join("工作"), "b.png", &png_bytes(2));
        write_file(&root, "note.txt", b"not an image");

        let r = import_paths(&conn, &lib, &[root]).unwrap();

        // 导入顺序可能因子目录先于直属文件而不同，这里只验证 hash 集合
        let mut actual = r.content_hashes.clone();
        let mut expected = vec![sha256_hex(&png_bytes(1)), sha256_hex(&png_bytes(2))];
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
        let cols = list_collections(&conn).unwrap();
        let names: Vec<_> = cols.iter().map(|c| c.name.clone()).collect();
        assert!(names.contains(&"猫猫".to_string()));
        assert!(names.contains(&"工作".to_string()));
        let cat = cols.iter().find(|c| c.name == "猫猫").unwrap();
        let in_cat = list_memes(&conn, &GalleryView::Collection(cat.id)).unwrap();
        assert_eq!(in_cat.len(), 1);
        assert_eq!(in_cat[0].original_filename, "a.png");
    }

    #[test]
    fn corrupt_image_fails_without_interrupting_others() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let conn = open_db(&lib).unwrap();
        let bad = write_file(tmp.path(), "fake.png", b"not a png at all");
        let good = write_file(tmp.path(), "good.png", &png_bytes(3));

        let r = import_paths(&conn, &lib, &[bad, good]).unwrap();

        assert_eq!(
            r,
            ImportResult {
                imported: 1,
                skipped: 0,
                unsupported: 0,
                failed: 1,
                content_hashes: vec![list_memes(&conn, &GalleryView::All).unwrap()[0]
                    .content_hash
                    .clone()]
            }
        );
        assert_eq!(
            list_memes(&conn, &GalleryView::All).unwrap()[0].original_filename,
            "good.png"
        );
    }

    // ---- 搜索（票 05）----

    /// 性能预算（spec）：10k 库搜索 p99 <50ms（release 讒）。debug 放宽 10 倍。
    #[test]
    fn perf_search_p99_on_10k_within_budget() {
        use std::time::{Duration, Instant};

        let tmp = tempfile::tempdir().unwrap();
        let conn = open_db(tmp.path()).unwrap();
        conn.execute_batch("BEGIN").unwrap();
        for i in 0..10_000usize {
            let hash = format!("{:064x}", i);
            let desc = if i % 7 == 0 {
                format!("这是一张无语的猫第{i}号")
            } else {
                format!("描述内容第{i}号")
            };
            conn.execute(
                "INSERT INTO meme (internal_path, original_filename, extension, mime_type, content_hash, imported_at, description)
                 VALUES (?1, ?2, 'png', 'image/png', ?3, 0, ?4)",
                params![format!("{hash}.png"), format!("表情包文件{i}.png"), hash, desc],
            )
            .unwrap();
        }
        conn.execute_batch("COMMIT").unwrap();

        let queries = [
            "猫",
            "无语",
            "表情包文件123",
            "猫 无语",
            "456 描述",
            "不存在的词",
        ];
        let mut durations = Vec::new();
        for q in queries.iter().cycle().take(100) {
            let start = Instant::now();
            let rows = search_memes(&conn, &GalleryView::All, q).unwrap();
            durations.push(start.elapsed());
            assert!(!rows.is_empty() || *q == "不存在的词" || *q == "猫 无语");
        }
        durations.sort();
        let p99 = durations[(durations.len() as f64 * 0.99) as usize - 1];
        println!(
            "10k 库搜索 p99: {p99:?}（max {:?}）",
            durations[durations.len() - 1]
        );
        let budget = if cfg!(debug_assertions) {
            Duration::from_millis(500)
        } else {
            Duration::from_millis(50)
        };
        assert!(p99 < budget, "搜索 p99 {p99:?} 超预算 {budget:?}");
    }

    /// 三张图：a（描述「很无语」+ 标签「猫」）、CAT.png（描述「办公图表」+ 标签「工作」）、
    /// c.png（描述「风景照」+ 标签「狗」）。返回 (conn, tempdir, [a, b, c] 的 meme id)。
    fn search_setup() -> (Connection, tempfile::TempDir, [i64; 3]) {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let conn = open_db(&lib).unwrap();
        let paths = [
            write_file(tmp.path(), "a.png", &png_bytes(1)),
            write_file(tmp.path(), "CAT.png", &png_bytes(2)),
            write_file(tmp.path(), "c.png", &png_bytes(3)),
        ];
        import_paths(&conn, &lib, &paths).unwrap();
        let memes = list_memes(&conn, &GalleryView::All).unwrap();
        let ids = [memes[0].id, memes[1].id, memes[2].id];
        crate::organize::set_description(&conn, ids[0], "很无语").unwrap();
        crate::organize::add_tag(&conn, ids[0], "猫").unwrap();
        crate::organize::set_description(&conn, ids[1], "办公图表").unwrap();
        crate::organize::add_tag(&conn, ids[1], "工作").unwrap();
        crate::organize::set_description(&conn, ids[2], "风景照").unwrap();
        crate::organize::add_tag(&conn, ids[2], "狗").unwrap();
        (conn, tmp, ids)
    }

    fn search_ids(conn: &Connection, view: &GalleryView, query: &str) -> Vec<i64> {
        search_memes(conn, view, query)
            .unwrap()
            .into_iter()
            .map(|m| m.id)
            .collect()
    }

    #[test]
    fn search_hits_filename_tags_and_description() {
        let (conn, _lib, ids) = search_setup();
        let [a, b, c] = ids;

        // 文件名（大小写不敏感）
        assert_eq!(search_ids(&conn, &GalleryView::All, "cat"), vec![b]);
        // 描述（中文子串）
        assert_eq!(search_ids(&conn, &GalleryView::All, "无语"), vec![a]);
        // 标签（a 的文件名与描述都不含「猫」）
        assert_eq!(search_ids(&conn, &GalleryView::All, "猫"), vec![a]);
        // 标签命中 c
        assert_eq!(search_ids(&conn, &GalleryView::All, "狗"), vec![c]);
    }

    #[test]
    fn search_multi_keyword_is_and_semantics() {
        let (conn, _lib, ids) = search_setup();
        let [a, b, _] = ids;

        // 两词同时命中同一张
        assert_eq!(search_ids(&conn, &GalleryView::All, "猫 无语"), vec![a]);
        // 文件名 + 标签跨字段 AND
        assert_eq!(search_ids(&conn, &GalleryView::All, "CAT 工作"), vec![b]);
        // 不存在同时含两词的图
        assert!(search_ids(&conn, &GalleryView::All, "猫 工作").is_empty());
    }

    #[test]
    fn search_ascii_case_insensitive_and_partial_match() {
        let (conn, _lib, ids) = search_setup();
        let [a, b, c] = ids;

        assert_eq!(search_ids(&conn, &GalleryView::All, "CAT"), vec![b]); // 大写命中小写
        assert_eq!(search_ids(&conn, &GalleryView::All, "语"), vec![a]); // 单字部分匹配
        assert_eq!(search_ids(&conn, &GalleryView::All, "PN"), vec![a, b, c]); // 后缀部分匹配
    }

    #[test]
    fn search_scopes_to_current_view() {
        let (conn, _lib, ids) = search_setup();
        let [a, b, _] = ids;

        let col = crate::organize::create_collection(&conn, "组").unwrap();
        for meme_id in [a, b] {
            conn.execute(
                "INSERT INTO meme_collection (meme_id, collection_id) VALUES (?1, ?2)",
                params![meme_id, col.id],
            )
            .unwrap();
        }
        crate::organize::set_favorite(&conn, b, true).unwrap();

        // c 也叫 *.png，但不在「组」里 → 收藏夹内搜索只出 a、b
        assert_eq!(
            search_ids(&conn, &GalleryView::Collection(col.id), "png"),
            vec![a, b]
        );
        // 收藏视图内搜索只出 b
        assert_eq!(search_ids(&conn, &GalleryView::Favorites, "png"), vec![b]);
    }

    #[test]
    fn search_empty_query_returns_whole_view() {
        let (conn, _lib, ids) = search_setup();
        let [a, b, c] = ids;

        assert_eq!(search_ids(&conn, &GalleryView::All, ""), vec![a, b, c]);
        assert_eq!(search_ids(&conn, &GalleryView::All, "   "), vec![a, b, c]);
        // 空查询尊重视图范围：最近使用为空
        assert!(search_ids(&conn, &GalleryView::Recent, "").is_empty());
    }

    #[test]
    fn search_treats_like_wildcards_as_literals() {
        let (conn, _lib, ids) = search_setup();
        let [a, _, _] = ids;

        // 「%」按字面量匹配，不应变成全表通配
        assert!(search_ids(&conn, &GalleryView::All, "%").is_empty());
        assert_eq!(search_ids(&conn, &GalleryView::All, "a.png"), vec![a]);
    }
}
