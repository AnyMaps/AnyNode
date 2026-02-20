use rusqlite::Row;
use serde::{Deserialize, Serialize};

/// Locality data from WhosOnFirst database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Locality {
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

impl Locality {
    /// Create a Locality from a SQLite row
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

/// Extended locality information including storage metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalityInfo {
    #[serde(flatten)]
    pub locality: Locality,
    pub file_size: u64,
    pub cid: String,
}

impl LocalityInfo {
    pub fn new(locality: Locality, file_size: u64, cid: String) -> Self {
        Self {
            locality,
            file_size,
            cid,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedLocalitiesResult {
    pub localities: Vec<LocalityInfo>,
    pub pagination: PaginationInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationInfo {
    pub page: u32,
    pub limit: u32,
    pub total: u32,
    pub total_pages: u32,
}
