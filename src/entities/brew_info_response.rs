use nestify::nest;
use serde::Deserialize;

nest! {
    #[derive(Debug, Deserialize)]
    pub struct BrewInfoResponse {
        pub formulae: Vec<
            #[derive(Debug, Deserialize)]
            pub struct BrewFormula {
                pub name: String,
                pub tap: Option<String>,
                pub desc: String,
                pub homepage: String,
                pub versions:
                    #[derive(Debug, Deserialize)]
                    pub struct BrewVersions {
                        pub stable: Option<String>,
                        pub head: Option<String>,
                    },
                pub installed: Vec<
                    #[derive(Debug, Deserialize)]
                    pub struct BrewInstalled {
                        pub version: String,
                        pub time: u64,
                        pub installed_as_dependency: bool,
                        pub installed_on_request: bool,
                    }
                >,
                pub outdated: bool,
                pub caveats: Option<String>,
            }
        >,
        pub casks: Vec<
            #[derive(Debug, Deserialize)]
            pub struct BrewCask {
                pub token: String,
                pub tap: Option<String>,
                pub name: Vec<String>,
                pub desc: Option<String>,
                pub homepage: String,
                pub version: String,
                pub installed: Option<String>,
                pub outdated: bool,
                pub caveats: Option<String>,
            }
        >,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_formula_with_null_tap() {
        let json = r#"{
            "formulae": [{
                "name": "test-formula",
                "tap": null,
                "desc": "Test description",
                "homepage": "https://example.com",
                "versions": {
                    "stable": "1.0.0",
                    "head": null
                },
                "installed": [],
                "outdated": false,
                "caveats": null
            }],
            "casks": []
        }"#;

        let result: Result<BrewInfoResponse, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert_eq!(response.formulae.len(), 1);
        assert_eq!(response.formulae[0].name, "test-formula");
        assert_eq!(response.formulae[0].tap, None);
    }

    #[test]
    fn test_deserialize_formula_with_tap() {
        let json = r#"{
            "formulae": [{
                "name": "test-formula",
                "tap": "homebrew/core",
                "desc": "Test description",
                "homepage": "https://example.com",
                "versions": {
                    "stable": "1.0.0",
                    "head": null
                },
                "installed": [],
                "outdated": false,
                "caveats": null
            }],
            "casks": []
        }"#;

        let result: Result<BrewInfoResponse, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert_eq!(response.formulae.len(), 1);
        assert_eq!(response.formulae[0].name, "test-formula");
        assert_eq!(response.formulae[0].tap, Some("homebrew/core".to_string()));
    }

    #[test]
    fn test_deserialize_cask_with_null_tap() {
        let json = r#"{
            "formulae": [],
            "casks": [{
                "token": "test-cask",
                "tap": null,
                "name": ["Test Cask"],
                "desc": "Test description",
                "homepage": "https://example.com",
                "version": "1.0.0",
                "installed": null,
                "outdated": false,
                "caveats": null
            }]
        }"#;

        let result: Result<BrewInfoResponse, _> = serde_json::from_str(json);
        assert!(result.is_ok());
        
        let response = result.unwrap();
        assert_eq!(response.casks.len(), 1);
        assert_eq!(response.casks[0].token, "test-cask");
        assert_eq!(response.casks[0].tap, None);
    }
}
