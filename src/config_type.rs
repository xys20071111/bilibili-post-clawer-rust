use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct MongoDBCollectionNames {
    pub posts: String,
    pub replies: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct MongoDBConfigure {
    pub uri: String,
    pub database: String,
    pub collections: MongoDBCollectionNames,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SourceStruct {
    pub name: String,
    pub id: u64,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Configure {
    pub headless: bool,
    pub browser_data_path: String,
    pub runtime_db_name: String,
    pub skip_recently_fetched_days: i32,
    pub exclude_fetched: bool,
    pub mongodb: MongoDBConfigure,
    pub sources: Vec<SourceStruct>,
}
