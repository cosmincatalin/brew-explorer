#[derive(Debug, Clone)]
pub struct MasApp {
    pub id: u32,
    pub name: String,
    pub version: String,
    pub available_version: Option<String>,
    pub outdated: bool,
}