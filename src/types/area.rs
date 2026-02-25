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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AreaInfo {
    #[serde(flatten)]
    pub area: AdministrativeArea,
    pub file_size: u64,
    pub cid: String,
}

impl AreaInfo {
    pub fn new(area: AdministrativeArea, file_size: u64, cid: String) -> Self {
        Self {
            area,
            file_size,
            cid,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedAreasResult {
    pub areas: Vec<AreaInfo>,
    pub pagination: PaginationInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationInfo {
    pub page: u32,
    pub limit: u32,
    pub total: u32,
    pub total_pages: u32,
}
