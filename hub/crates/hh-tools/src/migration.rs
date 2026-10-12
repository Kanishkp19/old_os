//! Offline identity and database migration commands.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use hh_auth::migration::{export_identity_bundle, import_identity_bundle};
use hh_db::Db;
use hh_storage::migration::StorageMigrationManifest;
use rusqlite::params;

#[derive(Subcommand)]
pub enum MigrationAction {
    /// Export encrypted identity bundle, SQLite snapshot, and manifest.
    Export {
        #[arg(long)]
        data_dir: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        passphrase: String,
    },
    /// Verify an encrypted identity bundle offline.
    Verify {
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        passphrase: String,
    },
    /// Import an encrypted identity bundle and SQLite snapshot.
    Import {
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        db_snapshot: PathBuf,
        #[arg(long)]
        target_dir: PathBuf,
        #[arg(long)]
        passphrase: String,
    },
}

pub async fn run(action: MigrationAction) -> Result<()> {
    match action {
        MigrationAction::Export {
            data_dir,
            output,
            passphrase,
        } => export(&data_dir, &output, &passphrase),
        MigrationAction::Verify { bundle, passphrase } => verify(&bundle, &passphrase),
        MigrationAction::Import {
            bundle,
            db_snapshot,
            target_dir,
            passphrase,
        } => import(&bundle, &db_snapshot, &target_dir, &passphrase),
    }
}

fn export(data_dir: &Path, output: &Path, passphrase: &str) -> Result<()> {
    fs::create_dir_all(output)?;
    let ca_cert = fs::read_to_string(data_dir.join("ca.cert.pem"))?;
    let server_cert = fs::read_to_string(data_dir.join("server.cert.pem"))?;
    let ca_key = String::from_utf8(hh_auth::ca::unprotect(&fs::read(
        data_dir.join("ca.key.enc"),
    )?)?)
    .context("CA key is not UTF-8 PEM")?;
    let server_key = String::from_utf8(hh_auth::ca::unprotect(&fs::read(
        data_dir.join("server.key.enc"),
    )?)?)
    .context("server key is not UTF-8 PEM")?;
    let db_path = data_dir.join("hub.db");
    if !db_path.is_file() {
        bail!("source hub.db does not exist");
    }
    let db = Db::open(&db_path)?;
    let (hub_id, hub_name) = db.hub_identity()?;
    let devices = db.list_devices()?;
    let bundle = export_identity_bundle(
        &hub_id,
        &hub_name,
        &ca_cert,
        &ca_key,
        &server_cert,
        &server_key,
        &devices,
        passphrase,
    )?;
    let snapshot_path = output.join("hub.db.snapshot");
    if snapshot_path.exists() {
        fs::remove_file(&snapshot_path)?;
    }
    {
        let connection = db.lock()?;
        connection.execute(
            "VACUUM INTO ?1",
            params![snapshot_path.to_string_lossy().as_ref()],
        )?;
    }
    let bundle_path = output.join("identity.bundle.enc");
    write_private(&bundle_path, &bundle)?;
    let storage = StorageMigrationManifest {
        version: 1,
        source_root_label: "database-only export".into(),
        target_root_id: String::new(),
        files: Vec::new(),
        total_bytes: 0,
        created_at_ms: hh_core::time::now_ms(),
    };
    let manifest = serde_json::json!({
        "version": 1,
        "hub_id": hub_id,
        "hub_name": hub_name,
        "device_count": devices.len(),
        "bundle_file": "identity.bundle.enc",
        "bundle_blake3": blake3::hash(&bundle).to_hex().to_string(),
        "database_file": "hub.db.snapshot",
        "database_blake3": blake3::hash(&fs::read(&snapshot_path)?).to_hex().to_string(),
        "storage": storage,
    });
    fs::write(
        output.join("migration_manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    println!("Export complete: bundle and database snapshot created.");
    Ok(())
}

fn verify(bundle: &Path, passphrase: &str) -> Result<()> {
    let payload = import_identity_bundle(&fs::read(bundle)?, passphrase)?;
    println!(
        "Verified hub {} ({}), {} devices.",
        payload.hub_id,
        payload.hub_name,
        payload.devices.len()
    );
    Ok(())
}

fn import(bundle: &Path, db_snapshot: &Path, target_dir: &Path, passphrase: &str) -> Result<()> {
    let payload = import_identity_bundle(&fs::read(bundle)?, passphrase)?;
    let connection = rusqlite::Connection::open_with_flags(
        db_snapshot,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let integrity: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        bail!("database snapshot failed integrity check");
    }
    let (db_id, db_name): (String, String) =
        connection.query_row("SELECT id, name FROM hub LIMIT 1", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
    if db_id != payload.hub_id || db_name != payload.hub_name {
        bail!("database snapshot does not match the identity bundle");
    }
    drop(connection);
    fs::create_dir_all(target_dir)?;
    for name in [
        "ca.cert.pem",
        "ca.key.enc",
        "server.cert.pem",
        "server.key.enc",
        "hub.db",
    ] {
        if target_dir.join(name).exists() {
            bail!("target identity or database already exists");
        }
    }
    let ca_key = hh_auth::ca::protect(payload.ca_key_pem.as_bytes())?;
    let server_key = hh_auth::ca::protect(payload.server_key_pem.as_bytes())?;
    fs::write(target_dir.join("ca.cert.pem"), payload.ca_cert_pem)?;
    write_private(&target_dir.join("ca.key.enc"), &ca_key)?;
    fs::write(target_dir.join("server.cert.pem"), payload.server_cert_pem)?;
    write_private(&target_dir.join("server.key.enc"), &server_key)?;
    fs::copy(db_snapshot, target_dir.join("hub.db"))?;
    println!("Import complete: identity and database snapshot restored.");
    Ok(())
}

fn write_private(path: &Path, data: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(data)?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hh_auth::HubIdentity;
    use hh_db::DeviceRow;

    fn fixture(source: &Path) -> Result<String> {
        fs::create_dir_all(source)?;
        let identity = HubIdentity::load_or_create(source, "Migration Test")?;
        let db = Db::open(&source.join("hub.db"))?;
        db.set_hub_name("Migration Test")?;
        let hub_id = db.hub_identity()?.0;
        db.insert_device(
            &DeviceRow {
                id: "test-device".into(),
                name: "Test Device".into(),
                platform: "android".into(),
                model: None,
                app_version: None,
                cert_serial: "test-serial".into(),
                cert_expires_at: 9_999_999_999_999,
                scopes: vec!["files".into()],
                paired_at: 1,
                last_seen_at: None,
                status: "active".into(),
            },
            &identity.server_cert_pem,
            "test-public-key",
        )?;
        Ok(hub_id)
    }

    #[tokio::test]
    async fn export_verify_import_roundtrip() -> Result<()> {
        let root =
            std::env::temp_dir().join(format!("hh-tools-migration-{}", rand::random::<u64>()));
        let source = root.join("source");
        let output = root.join("output");
        let target = root.join("target");
        let hub_id = fixture(&source)?;
        run(MigrationAction::Export {
            data_dir: source,
            output: output.clone(),
            passphrase: "migration-passphrase-test".into(),
        })
        .await?;
        let bundle = output.join("identity.bundle.enc");
        run(MigrationAction::Verify {
            bundle: bundle.clone(),
            passphrase: "migration-passphrase-test".into(),
        })
        .await?;
        assert!(run(MigrationAction::Verify {
            bundle: bundle.clone(),
            passphrase: "wrong-passphrase".into()
        })
        .await
        .is_err());
        run(MigrationAction::Import {
            bundle,
            db_snapshot: output.join("hub.db.snapshot"),
            target_dir: target.clone(),
            passphrase: "migration-passphrase-test".into(),
        })
        .await?;
        assert_eq!(Db::open(&target.join("hub.db"))?.hub_identity()?.0, hub_id);
        assert_eq!(Db::open(&target.join("hub.db"))?.list_devices()?.len(), 1);
        HubIdentity::load_or_create(&target, "Migration Test")?;
        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn script_fixture() -> Result<()> {
        let Ok(mode) = std::env::var("HH_MIGRATION_TEST_MODE") else {
            return Ok(());
        };
        let source = PathBuf::from(std::env::var("HH_MIGRATION_TEST_SOURCE")?);
        match mode.as_str() {
            "seed" => {
                fixture(&source)?;
            }
            "validate" => {
                let target = PathBuf::from(std::env::var("HH_MIGRATION_TEST_TARGET")?);
                let source_db = Db::open(&source.join("hub.db"))?;
                let target_db = Db::open(&target.join("hub.db"))?;
                assert_eq!(source_db.hub_identity()?, target_db.hub_identity()?);
                assert_eq!(source_db.list_devices()?, target_db.list_devices()?);
                HubIdentity::load_or_create(&target, "Migration Test")?;
            }
            _ => bail!("unknown fixture mode"),
        }
        Ok(())
    }
}
