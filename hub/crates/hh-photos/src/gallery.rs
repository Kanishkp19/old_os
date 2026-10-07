//! Gallery timeline (TRD §8): paginated by stable cursor
//! `(taken_at DESC, file_id)`, grouped Year → Month client-side via
//! `/photos/years`.

use hh_core::error::Result;
use rusqlite::params;
use serde::Serialize;

use crate::{db_e, PhotoService};

#[derive(Debug, Clone, Serialize)]
pub struct TimelineItem {
    pub file_id: String,
    pub type_: String,
    pub taken_at: i64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ms: Option<u64>,
    pub thumb_status: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct YearMonthCount {
    pub year: i32,
    pub month: u32,
    pub count: u64,
}

impl PhotoService {
    pub fn timeline(&self, cursor: Option<(i64, String)>, limit: u32) -> Result<(Vec<TimelineItem>, Option<String>)> {
        let limit = limit.clamp(1, 500) as i64;
        let c = self.db.lock()?;
        let (sql, vals): (String, Vec<Box<dyn rusqlite::ToSql>>) = match cursor {
            Some((ts, fid)) => (
                "SELECT m.file_id, m.type, m.taken_at, m.width, m.height, m.duration_ms, m.thumb_status, f.name
                 FROM media m JOIN files f ON f.id = m.file_id
                 WHERE f.deleted_at IS NULL AND (m.taken_at < ?1 OR (m.taken_at = ?1 AND m.file_id < ?2))
                 ORDER BY m.taken_at DESC, m.file_id DESC LIMIT ?3"
                    .to_string(),
                vec![Box::new(ts), Box::new(fid), Box::new(limit + 1)],
            ),
            None => (
                "SELECT m.file_id, m.type, m.taken_at, m.width, m.height, m.duration_ms, m.thumb_status, f.name
                 FROM media m JOIN files f ON f.id = m.file_id
                 WHERE f.deleted_at IS NULL
                 ORDER BY m.taken_at DESC, m.file_id DESC LIMIT ?1"
                    .to_string(),
                vec![Box::new(limit + 1)],
            ),
        };
        let mut st = c.prepare(&sql).map_err(db_e)?;
        let refs: Vec<&dyn rusqlite::ToSql> = vals.iter().map(|b| b.as_ref()).collect();
        let mut items: Vec<TimelineItem> = st
            .query_map(refs.as_slice(), |r| {
                Ok(TimelineItem {
                    file_id: r.get(0)?,
                    type_: r.get(1)?,
                    taken_at: r.get(2)?,
                    width: r.get::<_, Option<i64>>(3)?.map(|v| v as u32),
                    height: r.get::<_, Option<i64>>(4)?.map(|v| v as u32),
                    duration_ms: r.get::<_, Option<i64>>(5)?.map(|v| v as u64),
                    thumb_status: r.get(6)?,
                    name: r.get(7)?,
                })
            })
            .map_err(db_e)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_e)?;
        let next = if items.len() as i64 > limit {
            items.truncate(limit as usize);
            items.last().map(|i| format!("{}:{}", i.taken_at, i.file_id))
        } else {
            None
        };
        Ok((items, next))
    }

    /// Year → month counts for the sticky timeline headers (FR-5.4).
    pub fn years(&self) -> Result<Vec<YearMonthCount>> {
        let c = self.db.lock()?;
        let mut st = c
            .prepare(
                "SELECT CAST(strftime('%Y', m.taken_at/1000, 'unixepoch') AS INTEGER) AS y,
                        CAST(strftime('%m', m.taken_at/1000, 'unixepoch') AS INTEGER) AS mo,
                        COUNT(*)
                 FROM media m JOIN files f ON f.id = m.file_id
                 WHERE f.deleted_at IS NULL
                 GROUP BY y, mo ORDER BY y DESC, mo DESC",
            )
            .map_err(db_e)?;
        let rows = st
            .query_map([], |r| {
                Ok(YearMonthCount { year: r.get(0)?, month: r.get(1)?, count: r.get::<_, i64>(2)? as u64 })
            })
            .map_err(db_e)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_e)?;
        Ok(rows)
    }

    /// Record media metadata for a finalized file (called from complete hook).
    pub fn register_media(&self, file_id: &str, type_: &str, meta: &crate::meta::MediaMeta) -> Result<()> {
        let c = self.db.lock()?;
        c.execute(
            "INSERT OR REPLACE INTO media
             (file_id, type, taken_at, taken_at_source, width, height, duration_ms, orientation,
              camera_make, camera_model, gps_lat, gps_lon)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                file_id, type_, meta.taken_at, meta.taken_at_source,
                meta.width, meta.height, meta.duration_ms.map(|v| v as i64), meta.orientation,
                meta.camera_make, meta.camera_model, meta.gps_lat, meta.gps_lon
            ],
        )
        .map_err(db_e)?;
        Ok(())
    }
}
