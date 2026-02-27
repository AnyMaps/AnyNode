use crate::types::AdministrativeArea;
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
    pub async fn new(database_path: &str, create_cid_tables: bool) -> Result<Self, DatabaseError> {
        let conn = Connection::open(database_path)?;

        let service = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        if create_cid_tables {
            service.create_cid_tables().await?;
        }

        Ok(service)
    }

    async fn create_cid_tables(&self) -> Result<(), DatabaseError> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();

            let create_cid_table = r#"
            CREATE TABLE IF NOT EXISTS area_cids (
                country_code TEXT NOT NULL,
                area_id INTEGER NOT NULL,
                cid TEXT NOT NULL,
                upload_time DATETIME DEFAULT CURRENT_TIMESTAMP,
                file_size INTEGER,
                PRIMARY KEY (country_code, area_id)
            )
            "#;

            let create_cid_index = r#"
            CREATE INDEX IF NOT EXISTS idx_area_cids_lookup
            ON area_cids(country_code, area_id)
            "#;

            conn.execute(create_cid_table, [])?;
            conn.execute(create_cid_index, [])?;

            Ok::<(), DatabaseError>(())
        })
        .await?
    }

    pub async fn get_country_areas(
        &self,
        country_code: &str,
    ) -> Result<Vec<AdministrativeArea>, DatabaseError> {
        let conn = self.conn.clone();
        let country_code = country_code.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();

            let conditions = [
                "placetype IN ('region', 'county')",
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
            let rows = stmt.query_map([&country_code], |row| AdministrativeArea::from_row(row))?;

            let areas = rows.collect::<Result<Vec<_>, _>>()?;
            Ok(areas)
        })
        .await?
    }

    pub async fn get_country_area_count(
        &self,
        country_code: &str,
    ) -> Result<u32, DatabaseError> {
        let conn = self.conn.clone();
        let country_code = country_code.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();

            let conditions = [
                "placetype IN ('region', 'county')",
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

    pub async fn get_area_by_id(
        &self,
        area_id: i64,
    ) -> Result<Option<AdministrativeArea>, DatabaseError> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();

            let query = r#"
            SELECT id, name, country, placetype, latitude, longitude, min_longitude, min_latitude, max_longitude, max_latitude
            FROM spr
            WHERE id = ?1 AND placetype IN ('region', 'county') AND is_current = 1 AND is_deprecated = 0
            "#;

            let mut stmt = conn.prepare(query)?;
            let rows = stmt.query_map([&area_id], |row| AdministrativeArea::from_row(row))?;

            let areas: Result<Vec<_>, _> = rows.collect();
            match areas {
                Ok(area_vec) => Ok(area_vec.into_iter().next()),
                Err(e) => Err(DatabaseError::RusqliteError(e)),
            }
        })
        .await?
    }

    pub async fn get_areas_by_ids(
        &self,
        area_ids: &[u32],
    ) -> Result<Vec<AdministrativeArea>, DatabaseError> {
        if area_ids.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.conn.clone();
        let area_ids: Vec<i64> = area_ids.iter().map(|&id| id as i64).collect();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();

            let placeholders: Vec<String> = area_ids.iter().map(|_| "?".to_string()).collect();
            let placeholder_str = placeholders.join(",");

            let query_str = format!(
                "SELECT id, name, country, placetype, latitude, longitude, min_longitude, min_latitude, max_longitude, max_latitude \
                 FROM spr \
                 WHERE id IN ({}) AND placetype IN ('region', 'county') AND is_current = 1 AND is_deprecated = 0",
                placeholder_str
            );

            let mut stmt = conn.prepare(&query_str)?;
            let params: Vec<&dyn rusqlite::ToSql> = area_ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
            let rows = stmt.query_map(params.as_slice(), |row| AdministrativeArea::from_row(row))?;

            let areas = rows.collect::<Result<Vec<_>, _>>()?;
            Ok(areas)
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
            INSERT OR REPLACE INTO area_cids
            (country_code, area_id, cid, file_size, upload_time)
            VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)
            "#;

            for (country_code, area_id, cid, file_size) in mappings {
                let area_id_i64 = area_id as i64;
                let file_size_i64 = file_size as i64;
                tx.execute(
                    query,
                    rusqlite::params![
                        &country_code,
                        &area_id_i64,
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

    pub async fn has_cid_mapping(
        &self,
        country_code: &str,
        area_id: u32,
    ) -> Result<bool, DatabaseError> {
        let conn = self.conn.clone();
        let country_code = country_code.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();

            let query = r#"
            SELECT COUNT(*) as count FROM area_cids
            WHERE country_code = ?1 AND area_id = ?2
            "#;

            let area_id_i64 = area_id as i64;
            let count = conn.query_row(
                query,
                rusqlite::params![&country_code, &area_id_i64],
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

            let total_query = "SELECT COUNT(*) as count FROM area_cids";
            let total_count = conn.query_row(total_query, [], |row| row.get::<_, i64>(0))?;

            let countries_query = "SELECT COUNT(DISTINCT country_code) as count FROM area_cids";
            let countries_count = conn.query_row(countries_query, [], |row| row.get::<_, i64>(0))?;

            Ok((total_count as u64, countries_count as u64))
        })
        .await?
    }
}
