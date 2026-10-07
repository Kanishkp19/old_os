//! Library browsing/search (API_SPEC §6). Clients see logical categories,
//! never raw Windows paths (FR-4.1).

use hh_core::error::{Error, Result};
use hh_core::types::{FileObject, Page};
use rusqlite::params;

use crate::StorageService;

impl StorageService {
    /// Cursor-paginated listing. Cursor = created_at of the last item (stable
    /// with id tiebreak, same scheme as the gallery — TRD §8).
    pub fn list_files(
        &self,
        category: Option<&str>,
        query: Option<&str>,
        cursor: Option<i64>,
        limit: u32,
    ) -> Result<Page<FileObject>> {
        let limit = limit.clamp(1, 500) as i64;
        let c = self.db.lock()?;
        let mut sql = String::from(
            "SELECT id,name,category,mime,size,hash,created_at,modified_at,rel_path,last_verified_at
             FROM files WHERE deleted_at IS NULL",
        );
        let mut vals: Vec<Box<dyn rusqlite::ToSql>> = vec![];
        if let Some(cat) = category {
            sql.push_str(" AND category=?");
            vals.push(Box::new(cat.to_string()));
        }
        if let Some(q) = query {
            sql.push_str(" AND name LIKE ?");
            vals.push(Box::new(format!("%{}%", q.replace('%', ""))));
        }
        if let Some(cur) = cursor {
            sql.push_str(" AND created_at < ?");
            vals.push(Box::new(cur));
        }
        sql.push_str(" ORDER BY created_at DESC, id DESC LIMIT ?");
        vals.push(Box::new(limit + 1));

        let mut st = c.prepare(&sql).map_err(db_e)?;
        let refs: Vec<&dyn rusqlite::ToSql> = vals.iter().map(|b| b.as_ref()).collect();
        let mut items: Vec<FileObject> = st
            .query_map(refs.as_slice(), file_object)
            .map_err(db_e)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_e)?;
        let next_cursor = if items.len() as i64 > limit {
            items.truncate(limit as usize);
            items.last().map(|f| f.created_at.to_string())
        } else {
            None
        };
        Ok(Page { items, next_cursor })
    }

    pub fn get_file(&self, id: &str) -> Result<FileObject> {
        let c = self.db.lock()?;
        c.query_row(
            "SELECT id,name,category,mime,size,hash,created_at,modified_at,rel_path,last_verified_at
             FROM files WHERE id=?1 AND deleted_at IS NULL",
            params![id],
            file_object,
        )
        .map_err(|_| Error::NotFound(format!("file {id}")))
    }

    /// Absolute path on disk for content serving; jailed to the library.
    pub fn file_disk_path(&self, id: &str) -> Result<std::path::PathBuf> {
        let c = self.db.lock()?;
        let rel: String = c
            .query_row(
                "SELECT rel_path FROM files WHERE id=?1 AND deleted_at IS NULL",
                params![id],
                |r| r.get(0),
            )
            .map_err(|_| Error::NotFound(format!("file {id}")))?;
        drop(c);
        hh_core::paths::jail_join(&self.cfg.library_dir(), &rel)
    }

    /// Chunk manifest for download verification (API_SPEC §6).
    pub fn file_manifest(&self, id: &str) -> Result<Vec<(u64, String)>> {
        let c = self.db.lock()?;
        let mut st = c
            .prepare("SELECT idx, hash FROM file_chunks WHERE file_id=?1 ORDER BY idx")
            .map_err(db_e)?;
        let rows = st
            .query_map(params![id], |r| Ok((r.get::<_, i64>(0)? as u64, r.get::<_, String>(1)?)))
            .map_err(db_e)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_e)?;
        if rows.is_empty() {
            return Err(Error::NotFound(format!("manifest for {id}")));
        }
        Ok(rows)
    }

    pub fn rename_file(&self, id: &str, new_name: &str) -> Result<()> {
        let name = hh_core::paths::sanitize_component(new_name)?;
        let old_path = self.file_disk_path(id)?;
        let parent = old_path.parent().ok_or_else(|| Error::Internal("no parent".into()))?;
        let new_path = parent.join(&name);
        std::fs::rename(&old_path, &new_path)?;
        let new_rel = new_path
            .strip_prefix(self.cfg.library_dir())
            .map_err(|_| Error::Internal("rename escaped library".into()))?
            .to_string_lossy()
            .replace('\\', "/");
        let c = self.db.lock()?;
        c.execute(
            "UPDATE files SET name=?2, rel_path=?3 WHERE id=?1",
            params![id, name, new_rel],
        )
        .map_err(db_e)?;
        Ok(())
    }
}

pub(crate) fn db_e(e: rusqlite::Error) -> Error {
    Error::Db(e.to_string())
}

fn file_object(r: &rusqlite::Row<'_>) -> std::result::Result<FileObject, rusqlite::Error> {
    let rel: String = r.get(8)?;
    let path = rel.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
    Ok(FileObject {
        id: r.get(0)?,
        name: r.get(1)?,
        category: r.get(2)?,
        mime: r.get(3)?,
        size: r.get::<_, i64>(4)? as u64,
        hash: r.get(5)?,
        created_at: r.get(6)?,
        modified_at: r.get(7)?,
        path,
        last_verified_at: r.get(9)?,
    })
}
