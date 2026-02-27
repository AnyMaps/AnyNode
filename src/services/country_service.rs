use tracing::info;

const ALL_COUNTRIES: &[&str] = &[
    "AD", "AE", "AF", "AG", "AI", "AL", "AM", "AN", "AO", "AQ", "AR", "AS", "AT", "AU", "AW", "AX", "AZ",
    "BA", "BB", "BD", "BE", "BF", "BG", "BH", "BI", "BJ", "BL", "BM", "BN", "BO", "BQ", "BR", "BS", "BT",
    "BW", "BY", "BZ", "CA", "CC", "CD", "CF", "CG", "CH", "CI", "CK", "CL", "CM", "CN", "CO", "CR", "CU",
    "CV", "CW", "CX", "CY", "CZ", "DE", "DJ", "DK", "DM", "DO", "DZ", "EC", "EE", "EG", "EH", "ER", "ES",
    "ET", "FI", "FJ", "FK", "FM", "FO", "FR", "GA", "GB", "GD", "GE", "GF", "GG", "GH", "GI", "GL", "GM",
    "GN", "GP", "GQ", "GR", "GS", "GT", "GU", "GW", "GY", "HK", "HM", "HN", "HR", "HT", "HU", "ID", "IE",
    "IL", "IM", "IN", "IO", "IQ", "IR", "IS", "IT", "JE", "JM", "JO", "JP", "KE", "KG", "KH", "KI", "KM",
    "KN", "KP", "KR", "KW", "KY", "KZ", "LA", "LB", "LC", "LI", "LK", "LR", "LS", "LT", "LU", "LV", "LY",
    "MA", "MC", "MD", "ME", "MF", "MG", "MH", "MK", "ML", "MM", "MN", "MO", "MP", "MQ", "MR", "MS", "MT",
    "MU", "MV", "MW", "MX", "MY", "MZ", "NA", "NC", "NE", "NF", "NG", "NI", "NL", "NO", "NP", "NR", "NU",
    "NZ", "Nl", "OM", "PA", "PE", "PF", "PG", "PH", "PK", "PL", "PM", "PN", "PR", "PS", "PT", "PW", "PY",
    "QA", "RE", "RO", "RS", "RU", "RW", "SA", "SB", "SC", "SD", "SE", "SG", "SH", "SI", "SJ", "SK", "SL",
    "SM", "SN", "SO", "SR", "SS", "ST", "SV", "SX", "SY", "SZ", "TC", "TD", "TF", "TG", "TH", "TJ", "TK",
    "TL", "TM", "TN", "TO", "TR", "TT", "TV", "TW", "TZ", "UA", "UG", "UM", "UN", "US", "UY", "UZ", "VA",
    "VC", "VE", "VG", "VI", "VN", "VU", "WF", "WS", "XK", "XN", "XS", "XX", "XY", "XZ", "YE", "YT", "ZA",
    "ZM", "ZW",
];

pub struct CountryService;

impl Default for CountryService {
    fn default() -> Self {
        Self::new()
    }
}

impl CountryService {
    pub fn new() -> Self {
        Self
    }

    pub fn get_countries_to_process(&self, target_countries: &[String]) -> Vec<String> {
        if target_countries.is_empty() || target_countries.iter().any(|c| c == "ALL") {
            ALL_COUNTRIES.iter().map(|s| s.to_string()).collect()
        } else {
            let valid_countries: Vec<String> = target_countries
                .iter()
                .filter(|country| ALL_COUNTRIES.contains(&country.as_str()))
                .cloned()
                .collect();
            
            if valid_countries.len() != target_countries.len() {
                let invalid: Vec<_> = target_countries
                    .iter()
                    .filter(|c| !valid_countries.contains(c))
                    .collect();
                info!("Some requested countries not found in country list: {:?}", invalid);
            }
            
            valid_countries
        }
    }
}
