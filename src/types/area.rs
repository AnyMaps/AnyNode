use rusqlite::Row;
use serde::{Deserialize, Serialize};

/// Administrative area data from WhosOnFirst database (regions and counties)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdministrativeArea {
    pub id: i64,
    pub name: String,
    pub country: String,
    pub placetype: String,
    pub latitude: f64,
    pub longitude: f64,
    pub min_longitude: f64,
    pub min_latitude: f64,
    pub max_longitude: f64,
    pub max_latitude: f64,
}

impl AdministrativeArea {
    pub fn from_row(row: &Row) -> Result<Self, rusqlite::Error> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            country: row.get(2)?,
            placetype: row.get(3)?,
            latitude: row.get(4)?,
            longitude: row.get(5)?,
            min_longitude: row.get(6)?,
            min_latitude: row.get(7)?,
            max_longitude: row.get(8)?,
            max_latitude: row.get(9)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn create_test_area() -> AdministrativeArea {
        AdministrativeArea {
            id: 12345,
            name: "California".to_string(),
            country: "US".to_string(),
            placetype: "region".to_string(),
            latitude: 37.5,
            longitude: -119.5,
            min_longitude: -124.5,
            min_latitude: 32.5,
            max_longitude: -114.1,
            max_latitude: 42.0,
        }
    }

    #[test]
    fn administrative_area_fields() {
        let area = create_test_area();

        assert_eq!(area.id, 12345);
        assert_eq!(area.name, "California");
        assert_eq!(area.country, "US");
        assert_eq!(area.placetype, "region");
        assert_eq!(area.latitude, 37.5);
        assert_eq!(area.longitude, -119.5);
        assert_eq!(area.min_longitude, -124.5);
        assert_eq!(area.min_latitude, 32.5);
        assert_eq!(area.max_longitude, -114.1);
        assert_eq!(area.max_latitude, 42.0);
    }

    #[test]
    fn administrative_area_clone() {
        let area = create_test_area();
        let cloned = area.clone();

        assert_eq!(area.id, cloned.id);
        assert_eq!(area.name, cloned.name);
        assert_eq!(area.country, cloned.country);
        assert_eq!(area.placetype, cloned.placetype);
        assert_eq!(area.latitude, cloned.latitude);
        assert_eq!(area.longitude, cloned.longitude);
        assert_eq!(area.min_longitude, cloned.min_longitude);
        assert_eq!(area.min_latitude, cloned.min_latitude);
        assert_eq!(area.max_longitude, cloned.max_longitude);
        assert_eq!(area.max_latitude, cloned.max_latitude);
    }

    #[test]
    fn administrative_area_serialization() {
        let area = create_test_area();

        let json = serde_json::to_string(&area).expect("Failed to serialize");
        let deserialized: AdministrativeArea =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(area.id, deserialized.id);
        assert_eq!(area.name, deserialized.name);
        assert_eq!(area.country, deserialized.country);
        assert_eq!(area.placetype, deserialized.placetype);
        assert_eq!(area.latitude, deserialized.latitude);
        assert_eq!(area.longitude, deserialized.longitude);
        assert_eq!(area.min_longitude, deserialized.min_longitude);
        assert_eq!(area.min_latitude, deserialized.min_latitude);
        assert_eq!(area.max_longitude, deserialized.max_longitude);
        assert_eq!(area.max_latitude, deserialized.max_latitude);
    }

    #[test]
    fn administrative_area_from_row() {
        let conn = Connection::open_in_memory().expect("Failed to create in-memory database");

        conn.execute(
            "CREATE TABLE spr (
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
                is_current INTEGER,
                is_deprecated INTEGER
            )",
            [],
        )
        .expect("Failed to create table");

        conn.execute(
            "INSERT INTO spr (id, name, country, placetype, latitude, longitude, min_longitude, min_latitude, max_longitude, max_latitude, is_current, is_deprecated)
             VALUES (12345, 'California', 'US', 'region', 37.5, -119.5, -124.5, 32.5, -114.1, 42.0, 1, 0)",
            [],
        )
        .expect("Failed to insert test row");

        let area: AdministrativeArea = conn
            .query_row(
                "SELECT id, name, country, placetype, latitude, longitude, min_longitude, min_latitude, max_longitude, max_latitude FROM spr WHERE id = 12345",
                [],
                AdministrativeArea::from_row,
            )
            .expect("Failed to query row");

        assert_eq!(area.id, 12345);
        assert_eq!(area.name, "California");
        assert_eq!(area.country, "US");
        assert_eq!(area.placetype, "region");
        assert_eq!(area.latitude, 37.5);
        assert_eq!(area.longitude, -119.5);
        assert_eq!(area.min_longitude, -124.5);
        assert_eq!(area.min_latitude, 32.5);
        assert_eq!(area.max_longitude, -114.1);
        assert_eq!(area.max_latitude, 42.0);
    }
}
