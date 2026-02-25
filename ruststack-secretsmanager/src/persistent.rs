//! Secrets Manager persistent storage using SQLite

use parking_lot::Mutex;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Arc;

use crate::storage::{Secret, SecretVersion, SecretsManagerError};

/// Persistent storage for Secrets Manager
pub struct PersistentStorage {
    conn: Arc<Mutex<Connection>>,
}

impl PersistentStorage {
    pub fn new(data_dir: &Path) -> Result<Self, rusqlite::Error> {
        std::fs::create_dir_all(data_dir).ok();
        let db_path = data_dir.join("secretsmanager.db");
        let conn = Connection::open(db_path)?;

        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS secrets (
                name TEXT PRIMARY KEY,
                arn TEXT NOT NULL,
                description TEXT,
                kms_key_id TEXT,
                current_version_id TEXT,
                previous_version_id TEXT,
                created_date TEXT NOT NULL,
                last_changed_date TEXT NOT NULL,
                last_accessed_date TEXT,
                deleted_date TEXT,
                tags TEXT
            );

            CREATE TABLE IF NOT EXISTS secret_versions (
                name TEXT NOT NULL,
                version_id TEXT NOT NULL,
                secret_string TEXT,
                secret_binary TEXT,
                created_date TEXT NOT NULL,
                version_stages TEXT NOT NULL,
                PRIMARY KEY (name, version_id)
            );
            "#,
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn create_secret(
        &self,
        name: &str,
        description: Option<String>,
        kms_key_id: Option<String>,
        secret_string: Option<String>,
        secret_binary: Option<String>,
        tags: std::collections::HashMap<String, String>,
    ) -> Result<Secret, SecretsManagerError> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now();
        let version_id = uuid::Uuid::new_v4().to_string();
        let arn = format!(
            "arn:aws:secretsmanager:us-east-1:000000000000:secret:{}",
            name
        );

        // Insert secret
        conn.execute(
            "INSERT INTO secrets (name, arn, description, kms_key_id, current_version_id, created_date, last_changed_date, tags)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                name,
                arn,
                description,
                kms_key_id,
                version_id,
                now.to_rfc3339(),
                now.to_rfc3339(),
                serde_json::to_string(&tags).unwrap_or_default(),
            ],
        )
        .map_err(|e| SecretsManagerError::Internal(e.to_string()))?;

        // Insert version
        conn.execute(
            "INSERT INTO secret_versions (name, version_id, secret_string, secret_binary, created_date, version_stages)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                name,
                version_id,
                secret_string,
                secret_binary,
                now.to_rfc3339(),
                serde_json::to_string(&vec!["AWSCURRENT"]).unwrap(),
            ],
        )
        .map_err(|e| SecretsManagerError::Internal(e.to_string()))?;

        let secret = Secret {
            arn,
            name: name.to_string(),
            description,
            kms_key_id,
            versions: std::collections::HashMap::from([(
                version_id.clone(),
                SecretVersion {
                    version_id: version_id.clone(),
                    secret_string,
                    secret_binary,
                    created_date: now,
                    version_stages: vec!["AWSCURRENT".to_string()],
                },
            )]),
            current_version_id: Some(version_id),
            previous_version_id: None,
            created_date: now,
            last_changed_date: now,
            last_accessed_date: None,
            deleted_date: None,
            tags,
        };

        Ok(secret)
    }

    pub fn get_secret_value(
        &self,
        secret_id: &str,
        _version_id: Option<&str>,
        _version_stage: Option<&str>,
    ) -> Result<(Secret, SecretVersion), SecretsManagerError> {
        let conn = self.conn.lock();

        let secret: Secret = conn
            .query_row(
                "SELECT name, arn, description, kms_key_id, current_version_id, previous_version_id,
                        created_date, last_changed_date, last_accessed_date, deleted_date, tags
                 FROM secrets WHERE name = ?1",
                params![secret_id],
                |row| {
                    let tags_str: String = row.get(10)?;
                    let tags: std::collections::HashMap<String, String> =
                        serde_json::from_str(&tags_str).unwrap_or_default();

                    Ok(Secret {
                        name: row.get(0)?,
                        arn: row.get(1)?,
                        description: row.get(2)?,
                        kms_key_id: row.get(3)?,
                        current_version_id: row.get(4)?,
                        previous_version_id: row.get(5)?,
                        versions: std::collections::HashMap::new(),
                        created_date: row
                            .get::<_, String>(6)?
                            .parse()
                            .unwrap_or_else(|_| chrono::Utc::now()),
                        last_changed_date: row
                            .get::<_, String>(7)?
                            .parse()
                            .unwrap_or_else(|_| chrono::Utc::now()),
                        last_accessed_date: row
                            .get::<_, Option<String>>(8)?
                            .and_then(|s| s.parse().ok()),
                        deleted_date: row
                            .get::<_, Option<String>>(9)?
                            .and_then(|s| s.parse().ok()),
                        tags,
                    })
                },
            )
            .map_err(|_| SecretsManagerError::ResourceNotFound(secret_id.to_string()))?;

        let current_version_id = secret
            .current_version_id
            .as_ref()
            .ok_or_else(|| SecretsManagerError::ResourceNotFound(secret_id.to_string()))?;

        let version: SecretVersion = conn
            .query_row(
                "SELECT version_id, secret_string, secret_binary, created_date, version_stages
                 FROM secret_versions WHERE name = ?1 AND version_id = ?2",
                params![secret_id, current_version_id],
                |row| {
                    let stages_str: String = row.get(4)?;
                    Ok(SecretVersion {
                        version_id: row.get(0)?,
                        secret_string: row.get(1)?,
                        secret_binary: row.get(2)?,
                        created_date: row
                            .get::<_, String>(3)?
                            .parse()
                            .unwrap_or_else(|_| chrono::Utc::now()),
                        version_stages: serde_json::from_str(&stages_str).unwrap_or_default(),
                    })
                },
            )
            .map_err(|_| SecretsManagerError::ResourceNotFound(secret_id.to_string()))?;

        Ok((secret, version))
    }

    pub fn put_secret_value(
        &self,
        secret_id: &str,
        secret_string: Option<String>,
        secret_binary: Option<String>,
    ) -> Result<(Secret, SecretVersion), SecretsManagerError> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now();
        let new_version_id = uuid::Uuid::new_v4().to_string();

        let current_version_id: String = conn
            .query_row(
                "SELECT current_version_id FROM secrets WHERE name = ?1",
                params![secret_id],
                |row| row.get(0),
            )
            .map_err(|_| SecretsManagerError::ResourceNotFound(secret_id.to_string()))?;

        let previous_version_id: Option<String> = conn
            .query_row(
                "SELECT previous_version_id FROM secrets WHERE name = ?1",
                params![secret_id],
                |row| row.get(0),
            )
            .ok();

        conn.execute(
            "UPDATE secret_versions SET version_stages = ?1 WHERE name = ?2 AND version_id = ?3",
            params![
                serde_json::to_string(&vec!["AWSPREVIOUS"]).unwrap(),
                secret_id,
                current_version_id
            ],
        )
        .map_err(|e| SecretsManagerError::Internal(e.to_string()))?;

        if let Some(ref pvid) = previous_version_id {
            conn.execute(
                "UPDATE secret_versions SET version_stages = ?1 WHERE name = ?2 AND version_id = ?3",
                params![
                    serde_json::to_string(&Vec::<String>::new()).unwrap(),
                    secret_id,
                    pvid
                ],
            )
            .map_err(|e| SecretsManagerError::Internal(e.to_string()))?;
        }

        conn.execute(
            "INSERT INTO secret_versions (name, version_id, secret_string, secret_binary, created_date, version_stages)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                secret_id,
                new_version_id,
                secret_string.clone(),
                secret_binary.clone(),
                now.to_rfc3339(),
                serde_json::to_string(&vec!["AWSCURRENT"]).unwrap(),
            ],
        )
        .map_err(|e| SecretsManagerError::Internal(e.to_string()))?;

        conn.execute(
            "UPDATE secrets SET current_version_id = ?1, previous_version_id = ?2, last_changed_date = ?3 WHERE name = ?4",
            params![
                new_version_id,
                current_version_id,
                now.to_rfc3339(),
                secret_id,
            ],
        )
        .map_err(|e| SecretsManagerError::Internal(e.to_string()))?;

        let version = SecretVersion {
            version_id: new_version_id,
            secret_string,
            secret_binary,
            created_date: now,
            version_stages: vec!["AWSCURRENT".to_string()],
        };

        let secret: Secret = conn
            .query_row(
                "SELECT name, arn, description, kms_key_id, current_version_id, previous_version_id,
                        created_date, last_changed_date, last_accessed_date, deleted_date, tags
                 FROM secrets WHERE name = ?1",
                params![secret_id],
                |row| {
                    let tags_str: String = row.get(10)?;
                    let tags: std::collections::HashMap<String, String> =
                        serde_json::from_str(&tags_str).unwrap_or_default();

                    Ok(Secret {
                        name: row.get(0)?,
                        arn: row.get(1)?,
                        description: row.get(2)?,
                        kms_key_id: row.get(3)?,
                        current_version_id: row.get(4)?,
                        previous_version_id: row.get(5)?,
                        versions: std::collections::HashMap::new(),
                        created_date: row
                            .get::<_, String>(6)?
                            .parse()
                            .unwrap_or_else(|_| chrono::Utc::now()),
                        last_changed_date: row
                            .get::<_, String>(7)?
                            .parse()
                            .unwrap_or_else(|_| chrono::Utc::now()),
                        last_accessed_date: row
                            .get::<_, Option<String>>(8)?
                            .and_then(|s| s.parse().ok()),
                        deleted_date: row
                            .get::<_, Option<String>>(9)?
                            .and_then(|s| s.parse().ok()),
                        tags,
                    })
                },
            )
            .map_err(|_| SecretsManagerError::ResourceNotFound(secret_id.to_string()))?;

        Ok((secret, version))
    }

    pub fn delete_secret(
        &self,
        secret_id: &str,
        force_delete: bool,
    ) -> Result<Secret, SecretsManagerError> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now();

        if force_delete {
            // Get secret first
            let secret: Secret = conn
                .query_row(
                    "SELECT name, arn, description, kms_key_id, current_version_id, previous_version_id,
                            created_date, last_changed_date, last_accessed_date, deleted_date, tags
                     FROM secrets WHERE name = ?1",
                    params![secret_id],
                    |row| {
                        let tags_str: String = row.get(10)?;
                        let tags: std::collections::HashMap<String, String> =
                            serde_json::from_str(&tags_str).unwrap_or_default();

                        Ok(Secret {
                            name: row.get(0)?,
                            arn: row.get(1)?,
                            description: row.get(2)?,
                            kms_key_id: row.get(3)?,
                            current_version_id: row.get(4)?,
                            previous_version_id: row.get(5)?,
                            versions: std::collections::HashMap::new(),
                            created_date: row
                                .get::<_, String>(6)?
                                .parse()
                                .unwrap_or_else(|_| chrono::Utc::now()),
                            last_changed_date: row
                                .get::<_, String>(7)?
                                .parse()
                                .unwrap_or_else(|_| chrono::Utc::now()),
                            last_accessed_date: row
                                .get::<_, Option<String>>(8)?
                                .and_then(|s| s.parse().ok()),
                            deleted_date: Some(chrono::Utc::now()),
                            tags,
                        })
                    },
                )
                .map_err(|_| SecretsManagerError::ResourceNotFound(secret_id.to_string()))?;

            // Delete versions first
            conn.execute(
                "DELETE FROM secret_versions WHERE name = ?1",
                params![secret_id],
            )
            .map_err(|e| SecretsManagerError::Internal(e.to_string()))?;

            // Delete secret
            conn.execute("DELETE FROM secrets WHERE name = ?1", params![secret_id])
                .map_err(|e| SecretsManagerError::Internal(e.to_string()))?;

            Ok(secret)
        } else {
            // Soft delete - schedule for deletion in 30 days
            let deletion_date = now + chrono::Duration::days(30);

            conn.execute(
                "UPDATE secrets SET deleted_date = ?1, last_changed_date = ?2 WHERE name = ?3",
                params![deletion_date.to_rfc3339(), now.to_rfc3339(), secret_id,],
            )
            .map_err(|e| SecretsManagerError::Internal(e.to_string()))?;

            // Return the secret with updated deletion date
            let secret: Secret = conn
                .query_row(
                    "SELECT name, arn, description, kms_key_id, current_version_id, previous_version_id,
                            created_date, last_changed_date, last_accessed_date, deleted_date, tags
                     FROM secrets WHERE name = ?1",
                    params![secret_id],
                    |row| {
                        let tags_str: String = row.get(10)?;
                        let tags: std::collections::HashMap<String, String> =
                            serde_json::from_str(&tags_str).unwrap_or_default();

                        Ok(Secret {
                            name: row.get(0)?,
                            arn: row.get(1)?,
                            description: row.get(2)?,
                            kms_key_id: row.get(3)?,
                            current_version_id: row.get(4)?,
                            previous_version_id: row.get(5)?,
                            versions: std::collections::HashMap::new(),
                            created_date: row
                                .get::<_, String>(6)?
                                .parse()
                                .unwrap_or_else(|_| chrono::Utc::now()),
                            last_changed_date: row
                                .get::<_, String>(7)?
                                .parse()
                                .unwrap_or_else(|_| chrono::Utc::now()),
                            last_accessed_date: row
                                .get::<_, Option<String>>(8)?
                                .and_then(|s| s.parse().ok()),
                            deleted_date: row
                                .get::<_, Option<String>>(9)?
                                .and_then(|s| s.parse().ok()),
                            tags,
                        })
                    },
                )
                .map_err(|_| SecretsManagerError::ResourceNotFound(secret_id.to_string()))?;

            Ok(secret)
        }
    }

    pub fn list_secrets(&self) -> Vec<Secret> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT name, arn, description, current_version_id, created_date, last_changed_date, tags
                 FROM secrets ORDER BY name",
            )
            .unwrap();

        let secrets = stmt
            .query_map([], |row| {
                let tags_str: String = row.get(6)?;
                let tags: std::collections::HashMap<String, String> =
                    serde_json::from_str(&tags_str).unwrap_or_default();

                Ok(Secret {
                    name: row.get(0)?,
                    arn: row.get(1)?,
                    description: row.get(2)?,
                    kms_key_id: None,
                    current_version_id: row.get(3)?,
                    previous_version_id: None,
                    versions: std::collections::HashMap::new(),
                    created_date: row
                        .get::<_, String>(4)?
                        .parse()
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    last_changed_date: row
                        .get::<_, String>(5)?
                        .parse()
                        .unwrap_or_else(|_| chrono::Utc::now()),
                    last_accessed_date: None,
                    deleted_date: None,
                    tags,
                })
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        secrets
    }
}

impl crate::storage::SecretsManagerStorageTrait for PersistentStorage {
    fn create_secret(
        &self,
        name: &str,
        description: Option<String>,
        kms_key_id: Option<String>,
        secret_string: Option<String>,
        secret_binary: Option<String>,
        tags: std::collections::HashMap<String, String>,
    ) -> Result<Secret, SecretsManagerError> {
        Self::create_secret(
            self,
            name,
            description,
            kms_key_id,
            secret_string,
            secret_binary,
            tags,
        )
    }

    fn get_secret_value(
        &self,
        secret_id: &str,
        version_id: Option<&str>,
        version_stage: Option<&str>,
    ) -> Result<(Secret, SecretVersion), SecretsManagerError> {
        Self::get_secret_value(self, secret_id, version_id, version_stage)
    }

    fn put_secret_value(
        &self,
        secret_id: &str,
        secret_string: Option<String>,
        secret_binary: Option<String>,
    ) -> Result<(Secret, SecretVersion), SecretsManagerError> {
        Self::put_secret_value(self, secret_id, secret_string, secret_binary)
    }

    fn delete_secret(
        &self,
        secret_id: &str,
        force_delete: bool,
    ) -> Result<Secret, SecretsManagerError> {
        Self::delete_secret(self, secret_id, force_delete)
    }

    fn list_secrets(&self) -> Vec<Secret> {
        self.list_secrets()
    }

    fn describe_secret(&self, secret_id: &str) -> Result<Secret, SecretsManagerError> {
        let (secret, _) = self.get_secret_value(secret_id, None, None)?;
        Ok(secret)
    }
}
