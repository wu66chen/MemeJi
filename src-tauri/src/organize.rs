//! 整理能力：收藏夹 CRUD 与拖拽排序、标签规则、收藏状态、描述、删除图片。
//!
//! Seam：`create_collection` / `rename_collection` / `delete_collection` /
//! `reorder_collections` / `list_tags` / `add_tag` / `remove_tag` /
//! `set_favorite` / `set_description` / `touch_recent_at` / `delete_meme`。
//!
//! 系统收藏夹（全部/收藏/最近使用）是虚拟视图（见 `library::GalleryView`），
//! 不落 collection 表，因此天然固定且不可删；删除收藏夹只删关系，不删图。

use crate::thumbs;
use rusqlite::params;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Tag {
    pub id: i64,
    pub name: String,
}

#[derive(Debug)]
pub enum OrganizeError {
    Db(rusqlite::Error),
    Io(std::io::Error),
    Message(String),
}

impl std::fmt::Display for OrganizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrganizeError::Db(e) => f.write_str(&e.to_string()),
            OrganizeError::Io(e) => f.write_str(&e.to_string()),
            OrganizeError::Message(m) => f.write_str(m),
        }
    }
}
impl std::error::Error for OrganizeError {}
impl From<rusqlite::Error> for OrganizeError {
    fn from(e: rusqlite::Error) -> Self {
        OrganizeError::Db(e)
    }
}
impl From<std::io::Error> for OrganizeError {
    fn from(e: std::io::Error) -> Self {
        OrganizeError::Io(e)
    }
}

fn err<T>(msg: impl Into<String>) -> Result<T, OrganizeError> {
    Err(OrganizeError::Message(msg.into()))
}

// ---------------------------------------------------------------------------
// 收藏夹
// ---------------------------------------------------------------------------

pub fn create_collection(
    conn: &rusqlite::Connection,
    raw_name: &str,
) -> Result<crate::library::Collection, OrganizeError> {
    let name = raw_name.trim();
    if name.is_empty() {
        return err("收藏夹名称不能为空");
    }
    if collection_exists(conn, name)? {
        return err(format!("同名收藏夹已存在：{name}"));
    }
    conn.execute(
        "INSERT INTO collection (name, sort_order, created_at)
         VALUES (?1, (SELECT COALESCE(MAX(sort_order),0)+1 FROM collection), strftime('%s','now'))",
        params![name],
    )?;
    Ok(get_collection(conn, conn.last_insert_rowid())?)
}

pub fn rename_collection(
    conn: &rusqlite::Connection,
    id: i64,
    raw_name: &str,
) -> Result<(), OrganizeError> {
    let name = raw_name.trim();
    if name.is_empty() {
        return err("收藏夹名称不能为空");
    }
    if !collection_row_exists(conn, id)? {
        return err(format!("收藏夹不存在：{id}"));
    }
    if collection_exists(conn, name)? {
        return err(format!("同名收藏夹已存在：{name}"));
    }
    conn.execute("UPDATE collection SET name = ?1 WHERE id = ?2", params![name, id])?;
    Ok(())
}

/// 只删收藏夹与归属关系，图片本身与「全部」视图不受影响。
pub fn delete_collection(conn: &rusqlite::Connection, id: i64) -> Result<(), OrganizeError> {
    let deleted = conn.execute("DELETE FROM collection WHERE id = ?1", params![id])?;
    if deleted == 0 {
        return err(format!("收藏夹不存在：{id}"));
    }
    Ok(())
}

/// 拖拽排序：按前端给的新顺序重写 sort_order。
pub fn reorder_collections(
    conn: &rusqlite::Connection,
    ordered_ids: &[i64],
) -> Result<(), OrganizeError> {
    if ordered_ids.is_empty() { return Ok(()); }
    let group_id: Option<i64> = conn.query_row("SELECT group_id FROM collection WHERE id = ?1", params![ordered_ids[0]], |r| r.get(0))?;
    let expected: Vec<i64> = crate::library::list_collections(conn)?.into_iter()
        .filter(|c| c.group_id == group_id).map(|c| c.id).collect();
    let mut supplied = ordered_ids.to_vec();
    supplied.sort_unstable();
    let mut expected_sorted = expected;
    expected_sorted.sort_unstable();
    if supplied != expected_sorted { return err("排序必须包含同一分组内的全部收藏夹，且不能重复"); }
    conn.execute_batch("SAVEPOINT reorder_collections")?;
    for (index, id) in ordered_ids.iter().enumerate() {
        if let Err(e) = conn.execute(
            "UPDATE collection SET sort_order = ?1 WHERE id = ?2",
            params![(index + 1) as i64, id],
        ) {
            conn.execute_batch("ROLLBACK TO reorder_collections; RELEASE reorder_collections")?;
            return Err(e.into());
        }
    }
    conn.execute_batch("RELEASE reorder_collections")?;
    Ok(())
}

pub fn create_collection_group(conn: &rusqlite::Connection, raw_name: &str) -> Result<crate::library::CollectionGroup, OrganizeError> {
    let name = raw_name.trim();
    if name.is_empty() { return err("分组名称不能为空"); }
    conn.execute("INSERT INTO collection_group (name, sort_order, created_at) VALUES (?1, (SELECT COALESCE(MAX(sort_order),0)+1 FROM collection_group), strftime('%s','now'))", params![name])?;
    let id = conn.last_insert_rowid();
    Ok(crate::library::CollectionGroup { id, name: name.into(), sort_order: conn.query_row("SELECT sort_order FROM collection_group WHERE id = ?1", params![id], |r| r.get(0))? })
}

pub fn rename_collection_group(conn: &rusqlite::Connection, id: i64, raw_name: &str) -> Result<(), OrganizeError> {
    let name = raw_name.trim();
    if name.is_empty() { return err("分组名称不能为空"); }
    if conn.execute("UPDATE collection_group SET name = ?1 WHERE id = ?2", params![name, id])? == 0 { return err(format!("分组不存在：{id}")); }
    Ok(())
}

/// 删除分组后收藏夹回到未分组；收藏夹与表情关系都保留。
pub fn delete_collection_group(conn: &rusqlite::Connection, id: i64) -> Result<(), OrganizeError> {
    if conn.execute("DELETE FROM collection_group WHERE id = ?1", params![id])? == 0 { return err(format!("分组不存在：{id}")); }
    Ok(())
}

pub fn reorder_collection_groups(conn: &rusqlite::Connection, ids: &[i64]) -> Result<(), OrganizeError> {
    let mut expected: Vec<i64> = crate::library::list_collection_groups(conn)?.into_iter().map(|g| g.id).collect();
    let mut supplied = ids.to_vec();
    expected.sort_unstable();
    supplied.sort_unstable();
    if expected != supplied { return err("排序必须包含全部分组，且不能重复"); }
    conn.execute_batch("SAVEPOINT reorder_collection_groups")?;
    for (index, id) in ids.iter().enumerate() {
        if let Err(e) = conn.execute("UPDATE collection_group SET sort_order = ?1 WHERE id = ?2", params![(index + 1) as i64, id]) {
            conn.execute_batch("ROLLBACK TO reorder_collection_groups; RELEASE reorder_collection_groups")?;
            return Err(e.into());
        }
    }
    conn.execute_batch("RELEASE reorder_collection_groups")?;
    Ok(())
}

pub fn move_collection_to_group(conn: &rusqlite::Connection, id: i64, group_id: Option<i64>) -> Result<(), OrganizeError> {
    if let Some(group_id) = group_id {
        let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM collection_group WHERE id = ?1)", params![group_id], |r| r.get(0))?;
        if !exists { return err(format!("分组不存在：{group_id}")); }
    }
    let updated = conn.execute("UPDATE collection SET group_id = ?1, sort_order = (SELECT COALESCE(MAX(sort_order),0)+1 FROM collection WHERE group_id IS ?1 AND id <> ?2) WHERE id = ?2", params![group_id, id])?;
    if updated == 0 { return err(format!("收藏夹不存在：{id}")); }
    Ok(())
}

fn get_collection(
    conn: &rusqlite::Connection,
    id: i64,
) -> rusqlite::Result<crate::library::Collection> {
    conn.query_row(
        "SELECT id, name, sort_order, group_id FROM collection WHERE id = ?1",
        params![id],
        |r| Ok(crate::library::Collection { id: r.get(0)?, name: r.get(1)?, sort_order: r.get(2)?, group_id: r.get(3)? }),
    )
}

fn collection_exists(conn: &rusqlite::Connection, name: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM collection WHERE name = ?1)",
        params![name],
        |r| r.get(0),
    )
}

fn collection_row_exists(conn: &rusqlite::Connection, id: i64) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM collection WHERE id = ?1)",
        params![id],
        |r| r.get(0),
    )
}

// ---------------------------------------------------------------------------
// 标签：trim、大小写不敏感去重（normalized_name 键）、可选已有或输入新建
// ---------------------------------------------------------------------------

fn normalize_tag(raw: &str) -> String {
    raw.trim().to_lowercase()
}

pub fn list_tags(conn: &rusqlite::Connection) -> Result<Vec<Tag>, OrganizeError> {
    let mut stmt = conn.prepare("SELECT id, name FROM tag ORDER BY name COLLATE NOCASE")?;
    let rows = stmt
        .query_map([], |r| Ok(Tag { id: r.get(0)?, name: r.get(1)? }))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn tags_of_meme(conn: &rusqlite::Connection, meme_id: i64) -> Result<Vec<Tag>, OrganizeError> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name FROM meme_tag mt JOIN tag t ON t.id = mt.tag_id
         WHERE mt.meme_id = ?1 ORDER BY t.name COLLATE NOCASE",
    )?;
    let rows = stmt
        .query_map(params![meme_id], |r| {
            Ok(Tag { id: r.get(0)?, name: r.get(1)? })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// 加标签：命中已有（大小写不敏感）直接挂，否则新建。返回该图更新后的标签。
pub fn add_tag(
    conn: &rusqlite::Connection,
    meme_id: i64,
    raw_name: &str,
) -> Result<Vec<Tag>, OrganizeError> {
    let name = raw_name.trim();
    if name.is_empty() {
        return err("标签不能为空");
    }
    let normalized = normalize_tag(name);
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM tag WHERE normalized_name = ?1",
            params![normalized],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e: rusqlite::Error| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(OrganizeError::Db(other)),
        })?;
    let tag_id = match existing {
        Some(id) => id,
        None => {
            conn.execute(
                "INSERT INTO tag (name, normalized_name, created_at) VALUES (?1, ?2, strftime('%s','now'))",
                params![name, normalized],
            )?;
            conn.last_insert_rowid()
        }
    };
    conn.execute(
        "INSERT OR IGNORE INTO meme_tag (meme_id, tag_id) VALUES (?1, ?2)",
        params![meme_id, tag_id],
    )?;
    tags_of_meme(conn, meme_id)
}

/// 删标签关系；若该标签不再被任何图使用则清掉孤儿标签行。返回该图更新后的标签。
pub fn remove_tag(
    conn: &rusqlite::Connection,
    meme_id: i64,
    tag_id: i64,
) -> Result<Vec<Tag>, OrganizeError> {
    conn.execute(
        "DELETE FROM meme_tag WHERE meme_id = ?1 AND tag_id = ?2",
        params![meme_id, tag_id],
    )?;
    conn.execute(
        "DELETE FROM tag WHERE id = ?1 AND NOT EXISTS (
            SELECT 1 FROM meme_tag WHERE tag_id = ?1)",
        params![tag_id],
    )?;
    tags_of_meme(conn, meme_id)
}

// ---------------------------------------------------------------------------
// 收藏 / 描述 / 最近使用 / 删除
// ---------------------------------------------------------------------------

pub fn set_favorite(
    conn: &rusqlite::Connection,
    meme_id: i64,
    favorite: bool,
) -> Result<(), OrganizeError> {
    let updated =
        conn.execute("UPDATE meme SET is_favorite = ?1 WHERE id = ?2", params![favorite, meme_id])?;
    if updated == 0 {
        return err(format!("表情不存在：{meme_id}"));
    }
    Ok(())
}

pub fn set_description(
    conn: &rusqlite::Connection,
    meme_id: i64,
    description: &str,
) -> Result<(), OrganizeError> {
    let updated = conn.execute(
        "UPDATE meme SET description = ?1 WHERE id = ?2",
        params![description, meme_id],
    )?;
    if updated == 0 {
        return err(format!("表情不存在：{meme_id}"));
    }
    Ok(())
}

/// 记录一次复制（票 07 的 Smart Copy 会调用）；同一图反复复制只留最新时间。
pub fn touch_recent_at(
    conn: &rusqlite::Connection,
    meme_id: i64,
    used_at: i64,
) -> Result<(), OrganizeError> {
    conn.execute(
        "UPDATE meme SET last_used_at = ?1 WHERE id = ?2",
        params![used_at, meme_id],
    )?;
    Ok(())
}

/// 一张图归属的收藏夹（PRD 31.2：可加入多个收藏夹）。
pub fn collections_of_meme(
    conn: &rusqlite::Connection,
    meme_id: i64,
) -> Result<Vec<crate::library::Collection>, OrganizeError> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.name, c.sort_order, c.group_id FROM meme_collection mc
         JOIN collection c ON c.id = mc.collection_id
         WHERE mc.meme_id = ?1 ORDER BY c.sort_order, c.id",
    )?;
    let rows = stmt
        .query_map(params![meme_id], |r| {
            Ok(crate::library::Collection {
                id: r.get(0)?,
                name: r.get(1)?,
                sort_order: r.get(2)?,
                group_id: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// 把图加入收藏夹（可加入多个；重复加入幂等）。
pub fn add_meme_to_collection(
    conn: &rusqlite::Connection,
    meme_id: i64,
    collection_id: i64,
) -> Result<(), OrganizeError> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM meme WHERE id = ?1)",
        params![meme_id],
        |r| r.get(0),
    )?;
    if !exists {
        return err(format!("表情不存在：{meme_id}"));
    }
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM collection WHERE id = ?1)",
        params![collection_id],
        |r| r.get(0),
    )?;
    if !exists {
        return err(format!("收藏夹不存在：{collection_id}"));
    }
    conn.execute(
        "INSERT OR IGNORE INTO meme_collection (meme_id, collection_id) VALUES (?1, ?2)",
        params![meme_id, collection_id],
    )?;
    Ok(())
}

/// 把图移出收藏夹（图本身不动）。
pub fn remove_meme_from_collection(
    conn: &rusqlite::Connection,
    meme_id: i64,
    collection_id: i64,
) -> Result<(), OrganizeError> {
    conn.execute(
        "DELETE FROM meme_collection WHERE meme_id = ?1 AND collection_id = ?2",
        params![meme_id, collection_id],
    )?;
    Ok(())
}

/// 删除图片：连带删除库内文件与缩略图缓存。content_hash 在 meme 表上有
/// UNIQUE 约束，不存在其他图共享同一份缓存，可直接清理。
pub fn delete_meme(
    conn: &rusqlite::Connection,
    library_root: &Path,
    meme_id: i64,
) -> Result<(), OrganizeError> {
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT internal_path, content_hash FROM meme WHERE id = ?1",
            params![meme_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map(Some)
        .or_else(|e: rusqlite::Error| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(OrganizeError::Db(other)),
        })?;
    let Some((internal_path, content_hash)) = row else {
        return err(format!("表情不存在：{meme_id}"));
    };
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM meme WHERE id = ?1", params![meme_id])?;
    let internal = library_root.join(internal_path);
    // Stage the original on the same volume. A locked file leaves the database
    // transaction uncommitted; a failed commit restores the original filename.
    let staged = if internal.try_exists()? {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_nanos();
        let path = internal.with_extension(format!("deleting-{meme_id}-{nonce}"));
        std::fs::rename(&internal, &path)?;
        Some(path)
    } else { None };
    if let Err(e) = tx.commit() {
        if let Some(path) = &staged {
            std::fs::rename(path, &internal).map_err(|restore| OrganizeError::Message(format!("删除失败：{e}；恢复文件失败：{restore}（原文件暂存于 {}）", path.display())))?;
        }
        return Err(e.into());
    }
    if let Some(path) = staged {
        if let Err(e) = std::fs::remove_file(&path) {
            eprintln!("清理已删除图片的暂存文件失败 {}: {e}", path.display());
        }
    }
    let thumb = thumbs::thumbnail_path(&thumbs::cache_dir(library_root), &content_hash);
    if thumb.exists() {
        if let Err(e) = std::fs::remove_file(thumb) { eprintln!("清理已删除图片的缩略图失败: {e}"); }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 批量操作
// ---------------------------------------------------------------------------
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BatchAction {
    Favorite { favorite: bool },
    AddTag { name: String },
    AddToCollection { collection_id: i64 },
    MoveToCollection { source_id: i64, target_id: i64 },
    RemoveFromCollection { collection_id: i64 },
}

pub fn batch_edit(conn: &rusqlite::Connection, ids: &[i64], action: &BatchAction) -> Result<usize, OrganizeError> {
    let ids: std::collections::BTreeSet<i64> = ids.iter().copied().collect();
    if ids.is_empty() { return err("请先选择表情"); }
    let tx = conn.unchecked_transaction()?;
    for id in &ids {
        let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM meme WHERE id = ?1)", params![id], |r| r.get(0))?;
        if !exists { return err(format!("表情不存在：{id}，请刷新后重试")); }
    }
    let collection_ids = match action {
        BatchAction::AddToCollection { collection_id } | BatchAction::RemoveFromCollection { collection_id } => vec![*collection_id],
        BatchAction::MoveToCollection { source_id, target_id } => {
            if source_id == target_id { return err("目标与来源收藏夹相同"); }
            for id in &ids {
                let member: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM meme_collection WHERE meme_id = ?1 AND collection_id = ?2)", params![id,source_id], |r| r.get(0))?;
                if !member { return err(format!("表情 {id} 已不在来源收藏夹中，请刷新后重试")); }
            }
            vec![*source_id, *target_id]
        },
        _ => vec![],
    };
    for id in collection_ids {
        if !collection_row_exists(&tx, id)? { return err(format!("收藏夹不存在：{id}")); }
    }
    for id in &ids {
        match action {
            BatchAction::Favorite { favorite } => set_favorite(&tx,*id,*favorite)?,
            BatchAction::AddTag { name } => { add_tag(&tx,*id,name)?; },
            BatchAction::AddToCollection { collection_id } => add_meme_to_collection(&tx,*id,*collection_id)?,
            BatchAction::MoveToCollection { source_id, target_id } => {
                add_meme_to_collection(&tx,*id,*target_id)?;
                remove_meme_from_collection(&tx,*id,*source_id)?;
            },
            BatchAction::RemoveFromCollection { collection_id } => remove_meme_from_collection(&tx,*id,*collection_id)?,
        }
    }
    tx.commit()?;
    Ok(ids.len())
}

#[derive(Debug, serde::Serialize)]
pub struct DeleteFailure { pub id: i64, pub message: String }
#[derive(Debug, serde::Serialize)]
pub struct BatchDeleteResult { pub deleted_ids: Vec<i64>, pub failures: Vec<DeleteFailure> }

pub fn batch_delete(conn: &rusqlite::Connection, root: &Path, ids: &[i64]) -> BatchDeleteResult {
    let mut result = BatchDeleteResult { deleted_ids: vec![], failures: vec![] };
    for id in ids.iter().copied().collect::<std::collections::BTreeSet<_>>() {
        match delete_meme(conn, root, id) {
            Ok(()) => result.deleted_ids.push(id),
            Err(e) => result.failures.push(DeleteFailure { id, message: e.to_string() }),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{self, GalleryView, Meme};
    use std::path::PathBuf;

    fn setup() -> (rusqlite::Connection, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let conn = library::open_db(&lib).unwrap();
        (conn, tmp)
    }

    fn import_png(conn: &rusqlite::Connection, lib: &Path, name: &str, seed: u8) -> i64 {
        let img: image::RgbImage = image::ImageBuffer::from_fn(3, 3, |x, y| {
            image::Rgb([(x * 40 + seed as u32) as u8, (y * 40) as u8, 128])
        });
        let src = tmp_source(name, &library_png_bytes(&img));
        library::import_paths(conn, lib, &[src]).unwrap();
        library::list_memes(conn, &GalleryView::All)
            .unwrap()
            .iter()
            .find(|m| m.original_filename == name)
            .unwrap()
            .id
    }

    fn tmp_source(name: &str, bytes: &[u8]) -> PathBuf {
        let dir = std::env::temp_dir().join("organize-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }

    fn library_png_bytes(img: &image::RgbImage) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img.clone())
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    fn names(tags: &[Tag]) -> Vec<String> {
        tags.iter().map(|t| t.name.clone()).collect()
    }

    #[test]
    fn batch_move_preserves_other_memberships_and_deduplicates_ids() {
        let (conn, tmp) = setup();
        let a = import_png(&conn, &tmp.path().join("lib"), "bulk-move-a.png", 41);
        let b = import_png(&conn, &tmp.path().join("lib"), "bulk-move-b.png", 42);
        let source = create_collection(&conn, "来源").unwrap().id;
        let target = create_collection(&conn, "目标").unwrap().id;
        let other = create_collection(&conn, "其他").unwrap().id;
        for id in [a,b] { add_meme_to_collection(&conn,id,source).unwrap(); }
        add_meme_to_collection(&conn,a,other).unwrap();
        add_meme_to_collection(&conn,b,target).unwrap();
        assert_eq!(batch_edit(&conn,&[a,b,a],&BatchAction::MoveToCollection { source_id:source,target_id:target }).unwrap(),2);
        assert!(library::list_memes(&conn,&GalleryView::Collection(source)).unwrap().is_empty());
        assert_eq!(library::list_memes(&conn,&GalleryView::Collection(target)).unwrap().len(),2);
        assert_eq!(collections_of_meme(&conn,a).unwrap().len(),2);
        assert_eq!(library::list_memes(&conn,&GalleryView::All).unwrap().len(),2);
    }

    #[test]
    fn batch_move_rejects_missing_source_membership_without_changing_anything() {
        let (conn,tmp) = setup();
        let a = import_png(&conn,&tmp.path().join("lib"),"bulk-source-a.png",43);
        let b = import_png(&conn,&tmp.path().join("lib"),"bulk-source-b.png",44);
        let source = create_collection(&conn,"source").unwrap().id;
        let target = create_collection(&conn,"target").unwrap().id;
        add_meme_to_collection(&conn,a,source).unwrap();
        assert!(batch_edit(&conn,&[a,b],&BatchAction::MoveToCollection {source_id:source,target_id:target}).is_err());
        assert_eq!(collections_of_meme(&conn,a).unwrap()[0].id,source);
        assert!(collections_of_meme(&conn,b).unwrap().is_empty());
    }

    #[test]
    fn batch_edit_rolls_back_on_sql_failure_midway() {
        let (conn,tmp) = setup();
        let a = import_png(&conn,&tmp.path().join("lib"),"bulk-rollback-a.png",45);
        let b = import_png(&conn,&tmp.path().join("lib"),"bulk-rollback-b.png",46);
        let target = create_collection(&conn,"target").unwrap().id;
        conn.execute_batch(&format!("CREATE TRIGGER reject_second BEFORE INSERT ON meme_collection WHEN NEW.meme_id = {b} BEGIN SELECT RAISE(ABORT,'blocked'); END;")).unwrap();
        assert!(batch_edit(&conn,&[a,b],&BatchAction::AddToCollection {collection_id:target}).is_err());
        assert!(collections_of_meme(&conn,a).unwrap().is_empty());
        conn.execute_batch("DROP TRIGGER reject_second").unwrap();
        assert_eq!(batch_edit(&conn,&[a,b],&BatchAction::AddToCollection {collection_id:target}).unwrap(),2);
    }

    #[test]
    fn batch_favorites_tags_and_removal_validate_all_ids() {
        let (conn,tmp) = setup();
        let a = import_png(&conn,&tmp.path().join("lib"),"bulk-meta-a.png",47);
        let b = import_png(&conn,&tmp.path().join("lib"),"bulk-meta-b.png",48);
        assert!(batch_edit(&conn,&[a,999999],&BatchAction::Favorite {favorite:true}).is_err());
        assert!(library::list_memes(&conn,&GalleryView::Favorites).unwrap().is_empty());
        assert_eq!(batch_edit(&conn,&[a,b],&BatchAction::Favorite {favorite:true}).unwrap(),2);
        assert_eq!(library::list_memes(&conn,&GalleryView::Favorites).unwrap().len(),2);
        batch_edit(&conn,&[a,b],&BatchAction::AddTag {name:"  Cat  ".into()}).unwrap();
        batch_edit(&conn,&[a,b],&BatchAction::AddTag {name:"cat".into()}).unwrap();
        assert_eq!(tags_of_meme(&conn,a).unwrap().len(),1);
        assert_eq!(tags_of_meme(&conn,b).unwrap().len(),1);
        let c = create_collection(&conn,"remove").unwrap().id;
        batch_edit(&conn,&[a,b],&BatchAction::AddToCollection {collection_id:c}).unwrap();
        batch_edit(&conn,&[a,b],&BatchAction::RemoveFromCollection {collection_id:c}).unwrap();
        assert!(library::list_memes(&conn,&GalleryView::Collection(c)).unwrap().is_empty());
        assert_eq!(library::list_memes(&conn,&GalleryView::All).unwrap().len(),2);
    }

    #[test]
    fn batch_delete_reports_each_unique_item() {
        let (conn,tmp) = setup();
        let a = import_png(&conn,&tmp.path().join("lib"),"bulk-delete.png",49);
        let result = batch_delete(&conn,&tmp.path().join("lib"),&[a,a,999999]);
        assert_eq!(result.deleted_ids,vec![a]);
        assert_eq!(result.failures.len(),1);
        assert_eq!(result.failures[0].id,999999);
        assert!(library::list_memes(&conn,&GalleryView::All).unwrap().is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn batch_delete_keeps_locked_file_and_its_database_record() {
        use std::os::windows::fs::OpenOptionsExt;
        let (conn,tmp) = setup();
        let root = tmp.path().join("lib");
        let a = import_png(&conn,&root,"bulk-locked.png",50);
        let m = library::list_memes(&conn,&GalleryView::All).unwrap().remove(0);
        let locked = std::fs::OpenOptions::new().read(true).share_mode(0).open(root.join(&m.internal_path)).unwrap();
        let result = batch_delete(&conn,&root,&[a]);
        assert_eq!(result.failures.len(),1);
        assert_eq!(library::list_memes(&conn,&GalleryView::All).unwrap().len(),1);
        drop(locked);
        assert!(root.join(m.internal_path).exists());
    }

    #[test]
    fn collection_crud_and_delete_keeps_memes() {
        let (conn, tmp) = setup();
        let lib = tmp.path().join("lib");
        let a = import_png(&conn, &lib, "a.png", 1);
        let b = import_png(&conn, &lib, "b.png", 2);

        let col = create_collection(&conn, "  工作  ").unwrap();
        assert_eq!(col.name, "工作");
        for meme_id in [a, b] {
            conn.execute(
                "INSERT INTO meme_collection (meme_id, collection_id) VALUES (?1, ?2)",
                params![meme_id, col.id],
            )
            .unwrap();
        }
        assert_eq!(library::list_memes(&conn, &GalleryView::Collection(col.id)).unwrap().len(), 2);

        // 重命名 + 重名校验
        rename_collection(&conn, col.id, "摸鱼").unwrap();
        assert!(rename_collection(&conn, col.id, "摸鱼").is_err()); // 与自己同名
        assert!(rename_collection(&conn, 999, "x").is_err());
        let created = create_collection(&conn, "猫猫").unwrap();
        assert!(create_collection(&conn, " 猫猫 ").is_err()); // trim 后重名
        assert!(create_collection(&conn, "   ").is_err());
        let _ = created;

        // 删除收藏夹：图片仍在「全部」，关系消失
        delete_collection(&conn, col.id).unwrap();
        assert!(delete_collection(&conn, col.id).is_err());
        let all = library::list_memes(&conn, &GalleryView::All).unwrap();
        assert_eq!(all.len(), 2);
        let leftovers: i64 = conn
            .query_row("SELECT COUNT(*) FROM meme_collection", [], |r| r.get(0))
            .unwrap();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn reorder_collections_updates_sort_order() {
        let (conn, _tmp) = setup();
        let c1 = create_collection(&conn, "甲").unwrap();
        let c2 = create_collection(&conn, "乙").unwrap();
        let c3 = create_collection(&conn, "丙").unwrap();

        reorder_collections(&conn, &[c3.id, c1.id, c2.id]).unwrap();

        let names: Vec<String> =
            library::list_collections(&conn).unwrap().into_iter().map(|c| c.name).collect();
        assert_eq!(names, vec!["丙", "甲", "乙"]);
    }

    #[test]
    fn group_delete_ungroups_collections_without_losing_memes() {
        let (conn, tmp) = setup();
        let lib = tmp.path().join("lib");
        let meme = import_png(&conn, &lib, "grouped.png", 17);
        let collection = create_collection(&conn, "猫猫").unwrap();
        add_meme_to_collection(&conn, meme, collection.id).unwrap();
        let group = create_collection_group(&conn, "朋友").unwrap();
        move_collection_to_group(&conn, collection.id, Some(group.id)).unwrap();
        assert_eq!(library::list_collections(&conn).unwrap()[0].group_id, Some(group.id));

        delete_collection_group(&conn, group.id).unwrap();
        assert_eq!(library::list_collections(&conn).unwrap()[0].group_id, None);
        assert_eq!(library::list_memes(&conn, &GalleryView::Collection(collection.id)).unwrap().len(), 1);
    }

    #[test]
    fn collection_reorder_is_scoped_to_its_group() {
        let (conn, _tmp) = setup();
        let group = create_collection_group(&conn, "常用").unwrap();
        let root = create_collection(&conn, "根目录").unwrap();
        let first = create_collection(&conn, "甲").unwrap();
        let second = create_collection(&conn, "乙").unwrap();
        move_collection_to_group(&conn, first.id, Some(group.id)).unwrap();
        move_collection_to_group(&conn, second.id, Some(group.id)).unwrap();
        assert!(reorder_collections(&conn, &[second.id, root.id]).is_err());
        reorder_collections(&conn, &[second.id, first.id]).unwrap();
        let cols = library::list_collections(&conn).unwrap();
        let grouped: Vec<i64> = cols.iter().filter(|c| c.group_id == Some(group.id)).map(|c| c.id).collect();
        assert_eq!(grouped, vec![second.id, first.id]);
        assert_eq!(cols.iter().find(|c| c.id == root.id).unwrap().group_id, None);
    }

    #[test]
    fn tag_rules_trim_case_insensitive_dedup() {
        let (conn, tmp) = setup();
        let lib = tmp.path().join("lib");
        let meme = import_png(&conn, &lib, "m.png", 1);

        add_tag(&conn, meme, "  Cat  ").unwrap();
        let tags = add_tag(&conn, meme, "cat").unwrap(); // 同义不同形 → 去重
        assert_eq!(names(&tags), vec!["Cat"]);
        let tags = add_tag(&conn, meme, "CAT ").unwrap();
        assert_eq!(names(&tags), vec!["Cat"]);

        let total: i64 = conn.query_row("SELECT COUNT(*) FROM tag", [], |r| r.get(0)).unwrap();
        assert_eq!(total, 1); // 只有一行 tag，display name 保留首次输入
        assert!(add_tag(&conn, meme, "   ").is_err());
        assert_eq!(library::list_memes(&conn, &GalleryView::All).unwrap()[0].tags, vec!["Cat"]);
    }

    #[test]
    fn remove_tag_cleans_orphan_but_keeps_shared_tag() {
        let (conn, tmp) = setup();
        let lib = tmp.path().join("lib");
        let m1 = import_png(&conn, &lib, "1.png", 1);
        let m2 = import_png(&conn, &lib, "2.png", 2);

        let shared = add_tag(&conn, m1, "猫").unwrap()[0].clone();
        add_tag(&conn, m2, "猫").unwrap();
        let private = add_tag(&conn, m1, "工作").unwrap().iter().find(|t| t.name == "工作").unwrap().clone();

        // 移除一张图上的共享标签：另一张仍保留
        let after = remove_tag(&conn, m1, shared.id).unwrap();
        assert_eq!(names(&after), vec!["工作"]);
        assert_eq!(tags_of_meme(&conn, m2).unwrap()[0].name, "猫");

        // 移除最后一个引用：孤儿标签行被清理
        let _ = remove_tag(&conn, m1, private.id).unwrap();
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM tag", [], |r| r.get(0)).unwrap();
        assert_eq!(total, 1); // 只剩「猫」
    }

    #[test]
    fn favorite_state_reflects_in_favorites_view() {
        let (conn, tmp) = setup();
        let lib = tmp.path().join("lib");
        let a = import_png(&conn, &lib, "a.png", 1);
        let b = import_png(&conn, &lib, "b.png", 2);

        set_favorite(&conn, a, true).unwrap();
        let favorites = library::list_memes(&conn, &GalleryView::Favorites).unwrap();
        assert_eq!(favorites.len(), 1);
        assert_eq!(favorites[0].id, a);
        assert!(library::list_memes(&conn, &GalleryView::All).unwrap().iter().all(|m: &Meme| {
            if m.id == a { m.is_favorite } else { !m.is_favorite }
        }));

        set_favorite(&conn, a, false).unwrap();
        assert!(library::list_memes(&conn, &GalleryView::Favorites).unwrap().is_empty());
        assert!(set_favorite(&conn, 999, true).is_err());
        let _ = b;
    }

    #[test]
    fn recent_orders_desc_and_one_row_per_meme() {
        let (conn, tmp) = setup();
        let lib = tmp.path().join("lib");
        let a = import_png(&conn, &lib, "a.png", 1);
        let b = import_png(&conn, &lib, "b.png", 2);
        let c = import_png(&conn, &lib, "c.png", 3);

        // 初始最近使用为空
        assert!(library::list_memes(&conn, &GalleryView::Recent).unwrap().is_empty());

        touch_recent_at(&conn, a, 100).unwrap();
        touch_recent_at(&conn, b, 200).unwrap();
        touch_recent_at(&conn, c, 300).unwrap();
        // a 被再次复制：同一条只保留最新时间，且排到最前
        touch_recent_at(&conn, a, 400).unwrap();

        let recent = library::list_memes(&conn, &GalleryView::Recent).unwrap();
        let ids: Vec<i64> = recent.iter().map(|m| m.id).collect();
        assert_eq!(ids, vec![a, c, b]);
        assert_eq!(recent[0].last_used_at, Some(400));
        assert_eq!(recent.len(), 3);
    }

    #[test]
    fn meme_can_join_and_leave_multiple_collections() {
        let (conn, tmp) = setup();
        let lib = tmp.path().join("lib");
        let meme = import_png(&conn, &lib, "m.png", 1);
        let c1 = create_collection(&conn, "工作").unwrap();
        let c2 = create_collection(&conn, "猫猫").unwrap();

        add_meme_to_collection(&conn, meme, c1.id).unwrap();
        add_meme_to_collection(&conn, meme, c2.id).unwrap();
        // 重复加入幂等
        add_meme_to_collection(&conn, meme, c1.id).unwrap();

        let names: Vec<String> =
            collections_of_meme(&conn, meme).unwrap().into_iter().map(|c| c.name).collect();
        assert_eq!(names, vec!["工作", "猫猫"]);
        assert_eq!(
            library::list_memes(&conn, &GalleryView::Collection(c1.id)).unwrap().len(),
            1
        );
        assert!(add_meme_to_collection(&conn, meme, 999).is_err());
        assert!(add_meme_to_collection(&conn, 999, c1.id).is_err());

        // 移出后图仍在「全部」
        remove_meme_from_collection(&conn, meme, c1.id).unwrap();
        assert_eq!(collections_of_meme(&conn, meme).unwrap().len(), 1);
        assert_eq!(library::list_memes(&conn, &GalleryView::All).unwrap().len(), 1);
    }

    #[test]
    fn delete_meme_removes_row_files_and_thumbnail() {
        let (conn, tmp) = setup();
        let lib = tmp.path().join("lib");
        let meme = import_png(&conn, &lib, "x.png", 1);

        // 先有缩略图缓存
        let hash = library::list_memes(&conn, &GalleryView::All).unwrap()[0]
            .content_hash
            .clone();
        crate::thumbs::ensure_thumbnail(&conn, &lib, &hash).unwrap();
        let internal = library::find_internal_path(&conn, &hash).unwrap().unwrap();

        delete_meme(&conn, &lib, meme).unwrap();

        assert!(library::find_internal_path(&conn, &hash).unwrap().is_none());
        assert!(!lib.join(internal).exists());
        assert!(!crate::thumbs::thumbnail_path(&crate::thumbs::cache_dir(&lib), &hash).exists());
        assert!(library::list_memes(&conn, &GalleryView::All).unwrap().is_empty());
        assert!(delete_meme(&conn, &lib, meme).is_err());
    }
}
