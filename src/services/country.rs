use crate::types::CountryInfo;
use std::collections::HashMap;

pub struct CountryService {
    country_codes: HashMap<String, String>,
}

impl CountryService {
    pub fn new() -> Self {
        Self {
            country_codes: Self::create_default_country_codes(),
        }
    }

    /// Get the list of countries to process based on target countries
    /// If target_countries is empty or contains "ALL", returns all countries
    pub fn get_countries_to_process(&self, target_countries: &[String]) -> Vec<String> {
        if target_countries.is_empty() || target_countries.iter().any(|c| c == "ALL") {
            self.country_codes.keys().cloned().collect()
        } else {
            target_countries
                .iter()
                .filter(|country| self.country_codes.contains_key(*country))
                .cloned()
                .collect()
        }
    }

    pub fn get_country_name(&self, country_code: &str) -> Option<&String> {
        self.country_codes.get(country_code)
    }

    pub fn get_all_countries(&self) -> Vec<CountryInfo> {
        self.country_codes
            .iter()
            .map(|(code, name)| CountryInfo {
                country_code: code.clone(),
                country_name: name.clone(),
                locality_count: 0, // Will be populated by database query
            })
            .collect()
    }

    fn create_default_country_codes() -> HashMap<String, String> {
        let mut codes = HashMap::new();

        codes.insert("AD".to_string(), "Andorra".to_string());
        codes.insert("AE".to_string(), "United Arab Emirates".to_string());
        codes.insert("AF".to_string(), "Afghanistan".to_string());
        codes.insert("AG".to_string(), "Antigua and Barbuda".to_string());
        codes.insert("AI".to_string(), "Anguilla".to_string());
        codes.insert("AL".to_string(), "Albania".to_string());
        codes.insert("AM".to_string(), "Armenia".to_string());
        codes.insert("AO".to_string(), "Angola".to_string());
        codes.insert("AQ".to_string(), "Antarctica".to_string());
        codes.insert("AR".to_string(), "Argentina".to_string());
        codes.insert("AS".to_string(), "American Samoa".to_string());
        codes.insert("AT".to_string(), "Austria".to_string());
        codes.insert("AU".to_string(), "Australia".to_string());
        codes.insert("AW".to_string(), "Aruba".to_string());
        codes.insert("AX".to_string(), "Aland Islands".to_string());
        codes.insert("AZ".to_string(), "Azerbaijan".to_string());
        codes.insert("BA".to_string(), "Bosnia and Herzegovina".to_string());
        codes.insert("BB".to_string(), "Barbados".to_string());
        codes.insert("BD".to_string(), "Bangladesh".to_string());
        codes.insert("BE".to_string(), "Belgium".to_string());
        codes.insert("BF".to_string(), "Burkina Faso".to_string());
        codes.insert("BG".to_string(), "Bulgaria".to_string());
        codes.insert("BH".to_string(), "Bahrain".to_string());
        codes.insert("BI".to_string(), "Burundi".to_string());
        codes.insert("BJ".to_string(), "Benin".to_string());
        codes.insert("BL".to_string(), "Saint Barthelemy".to_string());
        codes.insert("BM".to_string(), "Bermuda".to_string());
        codes.insert("BN".to_string(), "Brunei Darussalam".to_string());
        codes.insert("BO".to_string(), "Bolivia".to_string());
        codes.insert("BQ".to_string(), "Bonaire, Sint Eustatius and Saba".to_string());
        codes.insert("BR".to_string(), "Brazil".to_string());
        codes.insert("BS".to_string(), "Bahamas".to_string());
        codes.insert("BT".to_string(), "Bhutan".to_string());
        codes.insert("BV".to_string(), "Bouvet Island".to_string());
        codes.insert("BW".to_string(), "Botswana".to_string());
        codes.insert("BY".to_string(), "Belarus".to_string());
        codes.insert("BZ".to_string(), "Belize".to_string());
        codes.insert("CA".to_string(), "Canada".to_string());
        codes.insert("CC".to_string(), "Cocos (Keeling) Islands".to_string());
        codes.insert("CD".to_string(), "Congo, Democratic Republic".to_string());
        codes.insert("CF".to_string(), "Central African Republic".to_string());
        codes.insert("CG".to_string(), "Congo".to_string());
        codes.insert("CH".to_string(), "Switzerland".to_string());
        codes.insert("CI".to_string(), "Cote D'Ivoire".to_string());
        codes.insert("CK".to_string(), "Cook Islands".to_string());
        codes.insert("CL".to_string(), "Chile".to_string());
        codes.insert("CM".to_string(), "Cameroon".to_string());
        codes.insert("CN".to_string(), "China".to_string());
        codes.insert("CO".to_string(), "Colombia".to_string());
        codes.insert("CR".to_string(), "Costa Rica".to_string());
        codes.insert("CU".to_string(), "Cuba".to_string());
        codes.insert("CV".to_string(), "Cape Verde".to_string());
        codes.insert("CW".to_string(), "Curacao".to_string());
        codes.insert("CX".to_string(), "Christmas Island".to_string());
        codes.insert("CY".to_string(), "Cyprus".to_string());
        codes.insert("CZ".to_string(), "Czech Republic".to_string());
        codes.insert("DE".to_string(), "Germany".to_string());
        codes.insert("DJ".to_string(), "Djibouti".to_string());
        codes.insert("DK".to_string(), "Denmark".to_string());
        codes.insert("DM".to_string(), "Dominica".to_string());
        codes.insert("DO".to_string(), "Dominican Republic".to_string());
        codes.insert("DZ".to_string(), "Algeria".to_string());
        codes.insert("EC".to_string(), "Ecuador".to_string());
        codes.insert("EE".to_string(), "Estonia".to_string());
        codes.insert("EG".to_string(), "Egypt".to_string());
        codes.insert("EH".to_string(), "Western Sahara".to_string());
        codes.insert("ER".to_string(), "Eritrea".to_string());
        codes.insert("ES".to_string(), "Spain".to_string());
        codes.insert("ET".to_string(), "Ethiopia".to_string());
        codes.insert("FI".to_string(), "Finland".to_string());
        codes.insert("FJ".to_string(), "Fiji".to_string());
        codes.insert("FK".to_string(), "Falkland Islands (Malvinas)".to_string());
        codes.insert("FM".to_string(), "Micronesia, Federated States Of".to_string());
        codes.insert("FO".to_string(), "Faroe Islands".to_string());
        codes.insert("FR".to_string(), "France".to_string());
        codes.insert("GA".to_string(), "Gabon".to_string());
        codes.insert("GB".to_string(), "United Kingdom".to_string());
        codes.insert("GD".to_string(), "Grenada".to_string());
        codes.insert("GE".to_string(), "Georgia".to_string());
        codes.insert("GF".to_string(), "French Guiana".to_string());
        codes.insert("GG".to_string(), "Guernsey".to_string());
        codes.insert("GH".to_string(), "Ghana".to_string());
        codes.insert("GI".to_string(), "Gibraltar".to_string());
        codes.insert("GL".to_string(), "Greenland".to_string());
        codes.insert("GM".to_string(), "Gambia".to_string());
        codes.insert("GN".to_string(), "Guinea".to_string());
        codes.insert("GP".to_string(), "Guadeloupe".to_string());
        codes.insert("GQ".to_string(), "Equatorial Guinea".to_string());
        codes.insert("GR".to_string(), "Greece".to_string());
        codes.insert("GS".to_string(), "South Georgia and the South Sandwich Islands".to_string());
        codes.insert("GT".to_string(), "Guatemala".to_string());
        codes.insert("GU".to_string(), "Guam".to_string());
        codes.insert("GW".to_string(), "Guinea-Bissau".to_string());
        codes.insert("GY".to_string(), "Guyana".to_string());
        codes.insert("HK".to_string(), "Hong Kong".to_string());
        codes.insert("HM".to_string(), "Heard Island and Mcdonald Islands".to_string());
        codes.insert("HN".to_string(), "Honduras".to_string());
        codes.insert("HR".to_string(), "Croatia".to_string());
        codes.insert("HT".to_string(), "Haiti".to_string());
        codes.insert("HU".to_string(), "Hungary".to_string());
        codes.insert("ID".to_string(), "Indonesia".to_string());
        codes.insert("IE".to_string(), "Ireland".to_string());
        codes.insert("IL".to_string(), "Israel".to_string());
        codes.insert("IM".to_string(), "Isle of Man".to_string());
        codes.insert("IN".to_string(), "India".to_string());
        codes.insert("IO".to_string(), "British Indian Ocean Territory".to_string());
        codes.insert("IQ".to_string(), "Iraq".to_string());
        codes.insert("IR".to_string(), "Iran, Islamic Republic Of".to_string());
        codes.insert("IS".to_string(), "Iceland".to_string());
        codes.insert("IT".to_string(), "Italy".to_string());
        codes.insert("JE".to_string(), "Jersey".to_string());
        codes.insert("JM".to_string(), "Jamaica".to_string());
        codes.insert("JO".to_string(), "Jordan".to_string());
        codes.insert("JP".to_string(), "Japan".to_string());
        codes.insert("KE".to_string(), "Kenya".to_string());
        codes.insert("KG".to_string(), "Kyrgyzstan".to_string());
        codes.insert("KH".to_string(), "Cambodia".to_string());
        codes.insert("KI".to_string(), "Kiribati".to_string());
        codes.insert("KM".to_string(), "Comoros".to_string());
        codes.insert("KN".to_string(), "Saint Kitts and Nevis".to_string());
        codes.insert("KP".to_string(), "North Korea".to_string());
        codes.insert("KR".to_string(), "South Korea".to_string());
        codes.insert("KW".to_string(), "Kuwait".to_string());
        codes.insert("KY".to_string(), "Cayman Islands".to_string());
        codes.insert("KZ".to_string(), "Kazakhstan".to_string());
        codes.insert("LA".to_string(), "Lao People's Democratic Republic".to_string());
        codes.insert("LB".to_string(), "Lebanon".to_string());
        codes.insert("LC".to_string(), "Saint Lucia".to_string());
        codes.insert("LI".to_string(), "Liechtenstein".to_string());
        codes.insert("LK".to_string(), "Sri Lanka".to_string());
        codes.insert("LR".to_string(), "Liberia".to_string());
        codes.insert("LS".to_string(), "Lesotho".to_string());
        codes.insert("LT".to_string(), "Lithuania".to_string());
        codes.insert("LU".to_string(), "Luxembourg".to_string());
        codes.insert("LV".to_string(), "Latvia".to_string());
        codes.insert("LY".to_string(), "Libyan Arab Jamahiriya".to_string());
        codes.insert("MA".to_string(), "Morocco".to_string());
        codes.insert("MC".to_string(), "Monaco".to_string());
        codes.insert("MD".to_string(), "Moldova".to_string());
        codes.insert("ME".to_string(), "Montenegro".to_string());
        codes.insert("MF".to_string(), "Saint Martin".to_string());
        codes.insert("MG".to_string(), "Madagascar".to_string());
        codes.insert("MH".to_string(), "Marshall Islands".to_string());
        codes.insert("MK".to_string(), "Macedonia".to_string());
        codes.insert("ML".to_string(), "Mali".to_string());
        codes.insert("MM".to_string(), "Myanmar".to_string());
        codes.insert("MN".to_string(), "Mongolia".to_string());
        codes.insert("MO".to_string(), "Macao".to_string());
        codes.insert("MP".to_string(), "Northern Mariana Islands".to_string());
        codes.insert("MQ".to_string(), "Martinique".to_string());
        codes.insert("MR".to_string(), "Mauritania".to_string());
        codes.insert("MS".to_string(), "Montserrat".to_string());
        codes.insert("MT".to_string(), "Malta".to_string());
        codes.insert("MU".to_string(), "Mauritius".to_string());
        codes.insert("MV".to_string(), "Maldives".to_string());
        codes.insert("MW".to_string(), "Malawi".to_string());
        codes.insert("MX".to_string(), "Mexico".to_string());
        codes.insert("MY".to_string(), "Malaysia".to_string());
        codes.insert("MZ".to_string(), "Mozambique".to_string());
        codes.insert("NA".to_string(), "Namibia".to_string());
        codes.insert("NC".to_string(), "New Caledonia".to_string());
        codes.insert("NE".to_string(), "Niger".to_string());
        codes.insert("NF".to_string(), "Norfolk Island".to_string());
        codes.insert("NG".to_string(), "Nigeria".to_string());
        codes.insert("NI".to_string(), "Nicaragua".to_string());
        codes.insert("NL".to_string(), "Netherlands".to_string());
        codes.insert("NO".to_string(), "Norway".to_string());
        codes.insert("NP".to_string(), "Nepal".to_string());
        codes.insert("NR".to_string(), "Nauru".to_string());
        codes.insert("NU".to_string(), "Niue".to_string());
        codes.insert("NZ".to_string(), "New Zealand".to_string());
        codes.insert("OM".to_string(), "Oman".to_string());
        codes.insert("PA".to_string(), "Panama".to_string());
        codes.insert("PE".to_string(), "Peru".to_string());
        codes.insert("PF".to_string(), "French Polynesia".to_string());
        codes.insert("PG".to_string(), "Papua New Guinea".to_string());
        codes.insert("PH".to_string(), "Philippines".to_string());
        codes.insert("PK".to_string(), "Pakistan".to_string());
        codes.insert("PL".to_string(), "Poland".to_string());
        codes.insert("PM".to_string(), "Saint Pierre and Miquelon".to_string());
        codes.insert("PN".to_string(), "Pitcairn".to_string());
        codes.insert("PR".to_string(), "Puerto Rico".to_string());
        codes.insert("PS".to_string(), "Palestinian Territory, Occupied".to_string());
        codes.insert("PT".to_string(), "Portugal".to_string());
        codes.insert("PW".to_string(), "Palau".to_string());
        codes.insert("PY".to_string(), "Paraguay".to_string());
        codes.insert("QA".to_string(), "Qatar".to_string());
        codes.insert("RE".to_string(), "Reunion".to_string());
        codes.insert("RO".to_string(), "Romania".to_string());
        codes.insert("RS".to_string(), "Serbia".to_string());
        codes.insert("RU".to_string(), "Russian Federation".to_string());
        codes.insert("RW".to_string(), "Rwanda".to_string());
        codes.insert("SA".to_string(), "Saudi Arabia".to_string());
        codes.insert("SB".to_string(), "Solomon Islands".to_string());
        codes.insert("SC".to_string(), "Seychelles".to_string());
        codes.insert("SD".to_string(), "Sudan".to_string());
        codes.insert("SE".to_string(), "Sweden".to_string());
        codes.insert("SG".to_string(), "Singapore".to_string());
        codes.insert("SH".to_string(), "Saint Helena".to_string());
        codes.insert("SI".to_string(), "Slovenia".to_string());
        codes.insert("SJ".to_string(), "Svalbard and Jan Mayen".to_string());
        codes.insert("SK".to_string(), "Slovakia".to_string());
        codes.insert("SL".to_string(), "Sierra Leone".to_string());
        codes.insert("SM".to_string(), "San Marino".to_string());
        codes.insert("SN".to_string(), "Senegal".to_string());
        codes.insert("SO".to_string(), "Somalia".to_string());
        codes.insert("SR".to_string(), "Suriname".to_string());
        codes.insert("SS".to_string(), "South Sudan".to_string());
        codes.insert("ST".to_string(), "Sao Tome and Principe".to_string());
        codes.insert("SV".to_string(), "El Salvador".to_string());
        codes.insert("SX".to_string(), "Sint Maarten (Dutch part)".to_string());
        codes.insert("SY".to_string(), "Syrian Arab Republic".to_string());
        codes.insert("SZ".to_string(), "Swaziland".to_string());
        codes.insert("TC".to_string(), "Turks and Caicos Islands".to_string());
        codes.insert("TD".to_string(), "Chad".to_string());
        codes.insert("TF".to_string(), "French Southern Territories".to_string());
        codes.insert("TG".to_string(), "Togo".to_string());
        codes.insert("TH".to_string(), "Thailand".to_string());
        codes.insert("TJ".to_string(), "Tajikistan".to_string());
        codes.insert("TK".to_string(), "Tokelau".to_string());
        codes.insert("TL".to_string(), "Timor-Leste".to_string());
        codes.insert("TM".to_string(), "Turkmenistan".to_string());
        codes.insert("TN".to_string(), "Tunisia".to_string());
        codes.insert("TO".to_string(), "Tonga".to_string());
        codes.insert("TR".to_string(), "Turkey".to_string());
        codes.insert("TT".to_string(), "Trinidad and Tobago".to_string());
        codes.insert("TV".to_string(), "Tuvalu".to_string());
        codes.insert("TW".to_string(), "Taiwan".to_string());
        codes.insert("TZ".to_string(), "Tanzania, United Republic of".to_string());
        codes.insert("UA".to_string(), "Ukraine".to_string());
        codes.insert("UG".to_string(), "Uganda".to_string());
        codes.insert("UM".to_string(), "United States Minor Outlying Islands".to_string());
        codes.insert("US".to_string(), "United States".to_string());
        codes.insert("UY".to_string(), "Uruguay".to_string());
        codes.insert("UZ".to_string(), "Uzbekistan".to_string());
        codes.insert("VA".to_string(), "Holy See (Vatican City State)".to_string());
        codes.insert("VC".to_string(), "Saint Vincent and the Grenadines".to_string());
        codes.insert("VE".to_string(), "Venezuela".to_string());
        codes.insert("VG".to_string(), "Virgin Islands, British".to_string());
        codes.insert("VI".to_string(), "Virgin Islands, U.S.".to_string());
        codes.insert("VN".to_string(), "Vietnam".to_string());
        codes.insert("VU".to_string(), "Vanuatu".to_string());
        codes.insert("WF".to_string(), "Wallis and Futuna".to_string());
        codes.insert("WS".to_string(), "Samoa".to_string());
        codes.insert("YE".to_string(), "Yemen".to_string());
        codes.insert("YT".to_string(), "Mayotte".to_string());
        codes.insert("ZA".to_string(), "South Africa".to_string());
        codes.insert("ZM".to_string(), "Zambia".to_string());
        codes.insert("ZW".to_string(), "Zimbabwe".to_string());

        codes
    }
}

impl Default for CountryService {
    fn default() -> Self {
        Self::new()
    }
}
