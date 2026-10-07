-- M5 pilot metrics. Run READ-ONLY against a hub's database:
--   sqlite3 -readonly "%ProgramData%\HomeHub\hub.db" < metrics.sql
-- Every query is prefixed with a label so pasted output stays self-describing.

-- Q1: unaided backup — did a backup source reach a fully-verified state?
-- (A household "completed" if ≥1 backup source has ≥50 verified items and
--  no items still pending or failed.)
SELECT 'Q1_backup_sources' AS metric,
       (SELECT COUNT(*) FROM backup_sources) AS sources,
       (SELECT COUNT(*) FROM backup_items WHERE status = 'verified') AS verified_items,
       (SELECT COUNT(*) FROM backup_items WHERE status IN ('pending','uploading','failed')) AS not_finished;

-- Q2: week-4 retention — any transfer or verification in the last 7 days?
SELECT 'Q2_recent_activity' AS metric,
       (SELECT COUNT(*) FROM transfers
         WHERE created_at > (strftime('%s','now') * 1000 - 7*24*3600*1000)) AS transfers_7d,
       (SELECT COUNT(*) FROM backup_items
         WHERE verified_at > (strftime('%s','now') * 1000 - 7*24*3600*1000)) AS verified_7d,
       (SELECT COUNT(*) FROM devices WHERE status = 'active') AS active_devices;

-- Q3: zero data loss — integrity problems vs. successful repairs.
SELECT 'Q3_integrity' AS metric,
       (SELECT COUNT(*) FROM integrity_events WHERE kind IN ('hash_mismatch','missing','read_error')) AS problems,
       (SELECT COUNT(*) FROM integrity_events WHERE kind = 'repaired_from_copy') AS repaired,
       (SELECT COUNT(*) FROM files WHERE id NOT IN (SELECT file_id FROM integrity_events WHERE file_id IS NOT NULL)) AS files_never_flagged;

-- Q4: footprint — rows give scale; RAM/CPU are measured externally
-- (Task Manager / typeperf on the hub laptop, 10-minute average, idle).
SELECT 'Q4_library_scale' AS metric,
       (SELECT COUNT(*) FROM files) AS files,
       (SELECT COALESCE(SUM(size), 0) FROM files) AS bytes_stored,
       (SELECT COUNT(*) FROM media) AS media_items,
       (SELECT COUNT(*) FROM transfers WHERE status = 'completed') AS completed_transfers;

-- Q5: failure surface — what went wrong most often?
SELECT 'Q5_failed_transfers' AS metric,
       error_code, COUNT(*) AS n
FROM transfers
WHERE status = 'failed'
GROUP BY error_code
ORDER BY n DESC;
