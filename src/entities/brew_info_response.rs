use nestify::nest;
use serde::Deserialize;

nest! {
    #[derive(Debug, Deserialize)]
    pub struct BrewInfoResponse {
        pub formulae: Vec<
            #[derive(Debug, Deserialize)]
            pub struct BrewFormula {
                pub name: String,
                pub tap: String,
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
                pub tap: String,
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