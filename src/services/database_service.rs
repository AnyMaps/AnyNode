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

    fn create_cid_tables_sync(conn: &Connection) -> Result<(), DatabaseError> {
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

        Ok(())
    }

    async fn create_cid_tables(&self) -> Result<(), DatabaseError> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.blocking_lock();
            Self::create_cid_tables_sync(&conn)?;
            Ok::<(), DatabaseError>(())
        })
        .await?
    }

    #[cfg(test)]
    pub fn from_connection(
        conn: Connection,
        create_cid_tables: bool,
    ) -> Result<Self, DatabaseError> {
        if create_cid_tables {
            Self::create_cid_tables_sync(&conn)?;
        }
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
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
            let rows = stmt.query_map([&country_code], AdministrativeArea::from_row)?;

            let areas = rows.collect::<Result<Vec<_>, _>>()?;
            Ok(areas)
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
            let rows = stmt.query_map([&area_id], AdministrativeArea::from_row)?;

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
            let rows = stmt.query_map(params.as_slice(), AdministrativeArea::from_row)?;

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
                    rusqlite::params![&country_code, &area_id_i64, &cid, &file_size_i64,],
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_spr_table(conn: &Connection) {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS spr (
                id INTEGER PRIMARY KEY,
                name TEXT,
                country TEXT,
                placetype TEXT,
                latitude REAL,
                longitude REAL,
                min_longitude REAL,
                min_latitude REAL,
                max_longitude REAL,
                max_latitude REAL,
                is_current INTEGER DEFAULT 1,
                is_deprecated INTEGER DEFAULT 0
            )",
            [],
        )
        .expect("Failed to create spr table");
    }

    fn insert_test_area(
        conn: &Connection,
        id: i64,
        name: &str,
        country: &str,
        placetype: &str,
        is_current: i32,
        is_deprecated: i32,
    ) {
        conn.execute(
            "INSERT INTO spr (id, name, country, placetype, latitude, longitude, min_longitude, min_latitude, max_longitude, max_latitude, is_current, is_deprecated)
             VALUES (?1, ?2, ?3, ?4, 37.5, -119.5, -124.5, 32.5, -114.1, 42.0, ?5, ?6)",
            rusqlite::params![id, name, country, placetype, is_current, is_deprecated],
        )
        .expect("Failed to insert test area");
    }

    fn create_test_db_with_spr_data() -> DatabaseService {
        let conn = Connection::open_in_memory().expect("Failed to create in-memory database");
        create_spr_table(&conn);

        insert_test_area(&conn, 1, "California", "US", "region", 1, 0);
        insert_test_area(&conn, 2, "Texas", "US", "region", 1, 0);
        insert_test_area(&conn, 3, "Los Angeles County", "US", "county", 1, 0);
        insert_test_area(&conn, 4, "Los Angeles", "US", "locality", 1, 0);
        insert_test_area(&conn, 5, "Deprecated Region", "US", "region", 1, 1);
        insert_test_area(&conn, 6, "Inactive Region", "US", "region", 0, 0);

        DatabaseService::from_connection(conn, true).expect("Failed to create DatabaseService")
    }

    #[tokio::test]
    async fn new_in_memory_no_tables() {
        let conn = Connection::open_in_memory().expect("Failed to create in-memory database");
        let service = DatabaseService::from_connection(conn, false);
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn new_in_memory_creates_tables() {
        let conn = Connection::open_in_memory().expect("Failed to create in-memory database");
        let service =
            DatabaseService::from_connection(conn, true).expect("Failed to create service");

        let exists: bool = tokio::task::spawn_blocking(move || {
            service
                .conn
                .blocking_lock()
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='area_cids')",
                    [],
                    |row| row.get(0),
                )
                .expect("Failed to check table existence")
        })
        .await
        .expect("spawn_blocking failed");
        assert!(exists);
    }

    #[tokio::test]
    async fn has_cid_mapping_false() {
        let conn = Connection::open_in_memory().expect("Failed to create in-memory database");
        let service =
            DatabaseService::from_connection(conn, true).expect("Failed to create service");

        let has_mapping = service
            .has_cid_mapping("US", 12345)
            .await
            .expect("has_cid_mapping failed");
        assert!(!has_mapping);
    }

    #[tokio::test]
    async fn has_cid_mapping_true() {
        let conn = Connection::open_in_memory().expect("Failed to create in-memory database");
        let service =
            DatabaseService::from_connection(conn, true).expect("Failed to create service");

        service
            .batch_insert_cid_mappings(&[("US".to_string(), 12345, "cid123".to_string(), 1024)])
            .await
            .expect("batch_insert failed");

        let has_mapping = service
            .has_cid_mapping("US", 12345)
            .await
            .expect("has_cid_mapping failed");
        assert!(has_mapping);
    }

    #[tokio::test]
    async fn batch_insert_cid_mappings() {
        let conn = Connection::open_in_memory().expect("Failed to create in-memory database");
        let service =
            DatabaseService::from_connection(conn, true).expect("Failed to create service");

        let mappings = vec![
            ("US".to_string(), 1, "cid1".to_string(), 100),
            ("US".to_string(), 2, "cid2".to_string(), 200),
            ("CA".to_string(), 3, "cid3".to_string(), 300),
        ];

        service
            .batch_insert_cid_mappings(&mappings)
            .await
            .expect("batch_insert failed");

        assert!(service
            .has_cid_mapping("US", 1)
            .await
            .expect("has_cid_mapping failed"));
        assert!(service
            .has_cid_mapping("US", 2)
            .await
            .expect("has_cid_mapping failed"));
        assert!(service
            .has_cid_mapping("CA", 3)
            .await
            .expect("has_cid_mapping failed"));
    }

    #[tokio::test]
    async fn batch_insert_cid_mappings_replaces() {
        let conn = Connection::open_in_memory().expect("Failed to create in-memory database");
        let service =
            DatabaseService::from_connection(conn, true).expect("Failed to create service");

        service
            .batch_insert_cid_mappings(&[("US".to_string(), 1, "cid_old".to_string(), 100)])
            .await
            .expect("batch_insert failed");

        service
            .batch_insert_cid_mappings(&[("US".to_string(), 1, "cid_new".to_string(), 200)])
            .await
            .expect("batch_insert failed");

        let (count, cid) = tokio::task::spawn_blocking(move || {
            let guard = service.conn.blocking_lock();
            let count: i64 = guard
                .query_row(
                    "SELECT COUNT(*) FROM area_cids WHERE country_code = 'US' AND area_id = 1",
                    [],
                    |row| row.get(0),
                )
                .expect("Failed to count rows");
            let cid: String = guard
                .query_row(
                    "SELECT cid FROM area_cids WHERE country_code = 'US' AND area_id = 1",
                    [],
                    |row| row.get(0),
                )
                .expect("Failed to get cid");
            (count, cid)
        })
        .await
        .expect("spawn_blocking failed");

        assert_eq!(count, 1);
        assert_eq!(cid, "cid_new");
    }

    #[tokio::test]
    async fn get_country_areas_empty_db() {
        let service = create_test_db_with_spr_data();

        let areas = service
            .get_country_areas("DE")
            .await
            .expect("get_country_areas failed");
        assert!(areas.is_empty());
    }

    #[tokio::test]
    async fn get_country_areas_returns_areas() {
        let service = create_test_db_with_spr_data();

        let areas = service
            .get_country_areas("US")
            .await
            .expect("get_country_areas failed");

        assert_eq!(areas.len(), 3);
        let names: Vec<&str> = areas.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"California"));
        assert!(names.contains(&"Texas"));
        assert!(names.contains(&"Los Angeles County"));
    }

    #[tokio::test]
    async fn get_country_areas_filters_placetype() {
        let service = create_test_db_with_spr_data();

        let areas = service
            .get_country_areas("US")
            .await
            .expect("get_country_areas failed");

        for area in &areas {
            assert!(area.placetype == "region" || area.placetype == "county");
            assert_ne!(area.placetype, "locality");
        }
    }

    #[tokio::test]
    async fn get_country_areas_filters_deprecated() {
        let service = create_test_db_with_spr_data();

        let areas = service
            .get_country_areas("US")
            .await
            .expect("get_country_areas failed");

        let names: Vec<&str> = areas.iter().map(|a| a.name.as_str()).collect();
        assert!(!names.contains(&"Deprecated Region"));
        assert!(!names.contains(&"Inactive Region"));
    }

    #[tokio::test]
    async fn get_area_by_id_found() {
        let service = create_test_db_with_spr_data();

        let area = service
            .get_area_by_id(1)
            .await
            .expect("get_area_by_id failed");
        assert!(area.is_some());

        let area = area.unwrap();
        assert_eq!(area.id, 1);
        assert_eq!(area.name, "California");
        assert_eq!(area.country, "US");
        assert_eq!(area.placetype, "region");
    }

    #[tokio::test]
    async fn get_area_by_id_not_found() {
        let service = create_test_db_with_spr_data();

        let area = service
            .get_area_by_id(99999)
            .await
            .expect("get_area_by_id failed");
        assert!(area.is_none());
    }

    #[tokio::test]
    async fn get_areas_by_ids_empty() {
        let service = create_test_db_with_spr_data();

        let areas = service
            .get_areas_by_ids(&[])
            .await
            .expect("get_areas_by_ids failed");
        assert!(areas.is_empty());
    }

    #[tokio::test]
    async fn get_areas_by_ids_multiple() {
        let service = create_test_db_with_spr_data();

        let areas = service
            .get_areas_by_ids(&[1, 2, 3])
            .await
            .expect("get_areas_by_ids failed");

        assert_eq!(areas.len(), 3);
        let ids: Vec<i64> = areas.iter().map(|a| a.id).collect();
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
        assert!(ids.contains(&3));
    }
}
