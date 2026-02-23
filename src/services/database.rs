use crate::types::Locality;
use rusqlite::Connection;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Rusqlite error: {0}")]
    RusqliteError(#[from] rusqlite::Error),
    #[error("Tokio join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub struct DatabaseService {
    conn: Arc<Mutex<Connection>>,
}

impl DatabaseService {
    /// Create a new database service
    /// For WhosOnFirst database, this opens the existing database
    /// For CID database, this creates the database and tables if needed
    pub async fn new(database_path: &str, create_cid_tables: bool) -> Result<Self, DatabaseError> {
        let conn = Connection::open(database_path)?;

        let service = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        // Create CID tables if requested (for CID mappings database)
        if create_cid_tables {
            service.create_cid_tables().await?;
        }

        Ok(service)
    }

    async fn create_cid_tables(&self) -> Result<(), DatabaseError> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();

            // Create CID mapping table
            let create_cid_table = r#"
            CREATE TABLE IF NOT EXISTS locality_cids (
                country_code TEXT NOT NULL,
                locality_id INTEGER NOT NULL,
                cid TEXT NOT NULL,
                upload_time DATETIME DEFAULT CURRENT_TIMESTAMP,
                file_size INTEGER,
                PRIMARY KEY (country_code, locality_id)
            )
            "#;

            // Index for fast CID lookups
            let create_cid_index = r#"
            CREATE INDEX IF NOT EXISTS idx_locality_cids_lookup
            ON locality_cids(country_code, locality_id)
            "#;

            conn.execute(create_cid_table, [])?;
            conn.execute(create_cid_index, [])?;

            Ok::<(), DatabaseError>(())
        })
        .await?
    }

    pub async fn get_country_localities(
        &self,
        country_code: &str,
    ) -> Result<Vec<Locality>, DatabaseError> {
        let conn = self.conn.clone();
        let country_code = country_code.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();

            let conditions = [
                "placetype = 'locality'",
                "is_current = 1",
                "is_deprecated = 0",
                "name IS NOT NULL",
                "name != ''",
                "latitude IS NOT NULL",
                "longitude IS NOT NULL",
                "min_longitude IS NOT NULL",
                "min_latitude IS NOT NULL",
                "max_longitude IS NOT NULL",
                "max_latitude IS NOT NULL",
                "country = ?1",
            ];

            let where_clause = conditions.join(" AND ");
            let query_str = format!(
                "SELECT id, name, country, placetype, latitude, longitude, min_longitude, min_latitude, max_longitude, max_latitude FROM spr WHERE {} ORDER BY id",
                where_clause
            );

            let mut stmt = conn.prepare(&query_str)?;
            let rows = stmt.query_map([&country_code], |row| Locality::from_row(row))?;

            let localities = rows.collect::<Result<Vec<_>, _>>()?;
            Ok(localities)
        })
        .await?
    }

    pub async fn get_country_locality_count(
        &self,
        country_code: &str,
    ) -> Result<u32, DatabaseError> {
        let conn = self.conn.clone();
        let country_code = country_code.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();

            let conditions = [
                "placetype = 'locality'",
                "is_current = 1",
                "is_deprecated = 0",
                "country = ?1",
            ];

            let where_clause = conditions.join(" AND ");
            let query_str = format!("SELECT COUNT(*) as count FROM spr WHERE {}", where_clause);

            let count = conn.query_row(&query_str, [&country_code], |row| row.get::<_, i64>(0))?;
            Ok(count as u32)
        })
        .await?
    }

    pub async fn get_locality_by_id(
        &self,
        locality_id: i64,
    ) -> Result<Option<Locality>, DatabaseError> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();

            let query = r#"
            SELECT id, name, country, placetype, latitude, longitude, min_longitude, min_latitude, max_longitude, max_latitude
            FROM spr
            WHERE id = ?1 AND placetype = 'locality' AND is_current = 1 AND is_deprecated = 0
            "#;

            let mut stmt = conn.prepare(query)?;
            let rows = stmt.query_map([&locality_id], |row| Locality::from_row(row))?;

            let localities: Result<Vec<_>, _> = rows.collect();
            match localities {
                Ok(locality_vec) => Ok(locality_vec.into_iter().next()),
                Err(e) => Err(DatabaseError::RusqliteError(e)),
            }
        })
        .await?
    }

    pub async fn get_localities_by_ids(
        &self,
        locality_ids: &[u32],
    ) -> Result<Vec<Locality>, DatabaseError> {
        if locality_ids.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.conn.clone();
        let locality_ids: Vec<i64> = locality_ids.iter().map(|&id| id as i64).collect();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();

            let placeholders: Vec<String> = locality_ids.iter().map(|_| "?".to_string()).collect();
            let placeholder_str = placeholders.join(",");

            let query_str = format!(
                "SELECT id, name, country, placetype, latitude, longitude, min_longitude, min_latitude, max_longitude, max_latitude \
                 FROM spr \
                 WHERE id IN ({}) AND placetype = 'locality' AND is_current = 1 AND is_deprecated = 0",
                placeholder_str
            );

            let mut stmt = conn.prepare(&query_str)?;
            let params: Vec<&dyn rusqlite::ToSql> = locality_ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
            let rows = stmt.query_map(params.as_slice(), |row| Locality::from_row(row))?;

            let localities = rows.collect::<Result<Vec<_>, _>>()?;
            Ok(localities)
        })
        .await?
    }

    pub async fn batch_insert_cid_mappings(
        &self,
        mappings: &[(String, u32, String, u64)],
    ) -> Result<(), DatabaseError> {
        let conn = self.conn.clone();
        let mappings = mappings.to_vec();

        tokio::task::spawn_blocking(move || {
            let mut conn = conn.blocking_lock();

            let tx = conn.transaction()?;

            let query = r#"
            INSERT OR REPLACE INTO locality_cids
            (country_code, locality_id, cid, file_size, upload_time)
            VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)
            "#;

            for (country_code, locality_id, cid, file_size) in mappings {
                // Convert u32/u64 to i64 for SQLite (doesn't support unsigned)
                let locality_id_i64 = locality_id as i64;
                let file_size_i64 = file_size as i64;
                tx.execute(
                    query,
                    rusqlite::params![
                        &country_code,
                        &locality_id_i64,
                        &cid,
                        &file_size_i64,
                    ],
                )?;
            }

            tx.commit()?;
            Ok(())
        })
        .await?
    }

    /// Check if a locality already has a CID mapping
    pub async fn has_cid_mapping(
        &self,
        country_code: &str,
        locality_id: u32,
    ) -> Result<bool, DatabaseError> {
        let conn = self.conn.clone();
        let country_code = country_code.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();

            let query = r#"
            SELECT COUNT(*) as count FROM locality_cids
            WHERE country_code = ?1 AND locality_id = ?2
            "#;

            let locality_id_i64 = locality_id as i64;
            let count = conn.query_row(
                query,
                rusqlite::params![&country_code, &locality_id_i64],
                |row| row.get::<_, i64>(0),
            )?;

            Ok(count > 0)
        })
        .await?
    }

    pub async fn get_cid_mapping_stats(&self) -> Result<(u64, u64), DatabaseError> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();

            // Get total mappings count
            let total_query = "SELECT COUNT(*) as count FROM locality_cids";
            let total_count = conn.query_row(total_query, [], |row| row.get::<_, i64>(0))?;

            // Get unique countries count
            let countries_query = "SELECT COUNT(DISTINCT country_code) as count FROM locality_cids";
            let countries_count = conn.query_row(countries_query, [], |row| row.get::<_, i64>(0))?;

            Ok((total_count as u64, countries_count as u64))
        })
        .await?
    }
}
