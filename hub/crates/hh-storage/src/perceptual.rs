//! Perceptual ("similar") duplicate detection (W2.3, FR-5.5): 64-bit dHash
//! from the thumbnail pipeline (stored in `media.phash`, indexed in migration
//! 0004) grouped by Hamming distance. Uses the existing `duplicate_groups`
//! table with kind `'similar'` (migration 0005 allows it) — no schema change.
//!
//! Never auto-deletes; resolution reuses the exact-duplicate resolver which
//! moves losers to trash.

use hh_core::error::Result;
use hh_core::time::now_ms;
use rusqlite::params;
use serde::Serialize;

use crate::{db_e, StorageService};

/// Hamming distance under which two photos count as near-duplicates.
pub const NEAR: u32 = 8;

/// Hamming distance between two 64-bit dHash values (0 = identical).
pub fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

fn candidate_groups(rows: Vec<(String, i64, i64)>) -> Vec<Vec<(String, u64, u64)>> {
    use std::collections::HashMap;
    let mut buckets: HashMap<i64, Vec<(String, u64, u64)>> = HashMap::new();
    for (fid, phash, size) in rows {
        let h = phash as u64;
        buckets
            .entry((h >> 48) as i64)
            .or_default()
            .push((fid, h, size as u64));
    }

    let mut groups = Vec::new();
    for mut members in buckets.into_values() {
        if members.len() < 2 {
            continue;
        }
        members.sort_by_key(|(_, h, _)| *h);
        let mut group = vec![members[0].clone()];
        for member in members.into_iter().skip(1) {
            let last = group.last().map(|(_, hash, _)| *hash).unwrap_or(member.1);
            if hamming(last, member.1) > NEAR {
                if group.len() >= 2 {
                    groups.push(std::mem::take(&mut group));
                }
                group.clear();
            }
            group.push(member);
        }
        if group.len() >= 2 {
            groups.push(group);
        }
    }
    groups
}

#[derive(Debug, Clone, Serialize)]
pub struct SimilarGroup {
    pub id: String,
    pub phash: String,
    pub reclaimable_bytes: u64,
    pub file_ids: Vec<String>,
    pub files: Vec<SimilarFile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SimilarFile {
    pub id: String,
    pub name: String,
}

impl StorageService {
    /// Scan media with a dHash and record `similar` duplicate groups.
    /// Photos are bucketed by the top 16 bits, so the O(n²) comparison only
    /// runs inside small buckets (documented approximation).
    pub fn scan_similar(&self) -> Result<u32> {
        let rows: Vec<(String, i64, i64)> = {
            let c = self.db.lock()?;
            let mut st = c
                .prepare(
                    "SELECT m.file_id, m.phash, COALESCE(f.size,0)
                     FROM media m JOIN files f ON f.id = m.file_id
                     WHERE m.phash IS NOT NULL AND f.deleted_at IS NULL",
                )
                .map_err(db_e)?;
            let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .map_err(db_e)?
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(db_e)?;
            rows
        };

        // Bucket by top 16 bits; compare pairs within each bucket only.
        let pairs = candidate_groups(rows);

        let mut created = 0;
        for group in pairs {
            let total: u64 = group.iter().map(|(_, _, s)| s).sum();
            let largest: u64 = group.iter().map(|(_, _, s)| *s).max().unwrap_or(0);
            let reclaimable = total.saturating_sub(largest); // keep one copy
            let example_phash = format!("{:016x}", group[0].1);

            // Skip if an open similar group already exists with these members.
            let exists: Option<String> = {
                let c = self.db.lock()?;
                let mut st = c
                    .prepare(
                        "SELECT g.id FROM duplicate_groups g
                         JOIN duplicate_members m ON m.group_id = g.id
                         WHERE g.kind='similar' AND g.status='open' AND m.file_id=?1",
                    )
                    .map_err(db_e)?;
                st.query_row(params![group[0].0], |r| r.get(0)).ok()
            };
            if exists.is_some() {
                continue;
            }

            let gid = ulid::Ulid::new().to_string();
            let mut c = self.db.lock()?;
            let tx = c.transaction().map_err(db_e)?;
            tx.execute(
                "INSERT INTO duplicate_groups (id, kind, hash, reclaimable_bytes, detected_at)
                 VALUES (?1,'similar',?2,?3,?4)",
                params![gid, example_phash, reclaimable as i64, now_ms()],
            )
            .map_err(db_e)?;
            for (i, (fid, _, _)) in group.iter().enumerate() {
                tx.execute(
                    "INSERT INTO duplicate_members (group_id, file_id, is_keeper) VALUES (?1,?2,?3)",
                    params![gid, fid, (i == 0) as i64],
                )
                .map_err(db_e)?;
            }
            tx.commit().map_err(db_e)?;
            created += 1;
        }
        Ok(created)
    }

    /// Open similar-photo groups (same shape as the exact-duplicate list).
    pub fn list_similar(&self) -> Result<Vec<SimilarGroup>> {
        let c = self.db.lock()?;
        let mut st = c
            .prepare("SELECT id, COALESCE(hash,''), reclaimable_bytes FROM duplicate_groups WHERE kind='similar' AND status='open' ORDER BY detected_at DESC")
            .map_err(db_e)?;
        let mut groups: Vec<SimilarGroup> = st
            .query_map([], |r| {
                Ok(SimilarGroup {
                    id: r.get(0)?,
                    phash: r.get(1)?,
                    reclaimable_bytes: r.get::<_, i64>(2)? as u64,
                    file_ids: vec![],
                    files: vec![],
                })
            })
            .map_err(db_e)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_e)?;
        for g in &mut groups {
            let mut st = c.prepare("SELECT f.id,f.name FROM duplicate_members m JOIN files f ON f.id=m.file_id WHERE m.group_id=?1 AND f.deleted_at IS NULL").map_err(db_e)?;
            g.files = st
                .query_map(params![g.id], |r| Ok(SimilarFile{id:r.get(0)?,name:r.get(1)?}))
                .map_err(db_e)?
                .collect::<std::result::Result<Vec<SimilarFile>, _>>()
                .map_err(db_e)?;
            g.file_ids=g.files.iter().map(|f|f.id.clone()).collect();
        }
        Ok(groups)
    }
}

#[cfg(test)]
mod tests {
    use super::{candidate_groups, hamming};

    #[test]
    fn hamming_distance() {
        assert_eq!(hamming(0, 0), 0);
        assert_eq!(hamming(0, u64::MAX), 64);
        assert_eq!(hamming(0b1010, 0b0110), 2);
    }

    #[test]
    fn split_boundaries_never_offer_single_photos_as_duplicates() {
        let groups = candidate_groups(vec![
            ("first".into(), 0, 10),
            ("second".into(), 1, 12),
            ("unrelated".into(), 0xffff, 8),
            ("other_bucket".into(), 1_i64 << 48, 7),
        ]);
        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups[0]
                .iter()
                .map(|(id, _, _)| id.as_str())
                .collect::<Vec<_>>(),
            vec!["first", "second"]
        );
    }
}
