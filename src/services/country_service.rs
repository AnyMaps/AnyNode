use crate::services::DatabaseService;
use std::sync::Arc;
use tracing::info;

pub struct CountryService {
    db: Arc<DatabaseService>,
}

impl CountryService {
    pub fn new(db: Arc<DatabaseService>) -> Self {
        Self { db }
    }

    pub async fn get_countries_to_process(&self, target_countries: &[String]) -> Result<Vec<String>, crate::services::DatabaseError> {
        if target_countries.is_empty() || target_countries.iter().any(|c| c == "ALL") {
            self.db.get_all_countries().await
        } else {
            let all_countries = self.db.get_all_countries().await?;
            let valid_countries: Vec<String> = target_countries
                .iter()
                .filter(|country| all_countries.contains(country))
                .cloned()
                .collect();
            
            if valid_countries.len() != target_countries.len() {
                let invalid: Vec<_> = target_countries
                    .iter()
                    .filter(|c| !valid_countries.contains(c))
                    .collect();
                info!("Some requested countries not found in database: {:?}", invalid);
            }
            
            Ok(valid_countries)
        }
    }
}
