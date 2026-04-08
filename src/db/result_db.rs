use bson::{DateTime, Document, doc, oid::ObjectId};
use futures_util::TryStreamExt;
use mongodb::{
    Client, Collection, Cursor, IndexModel,
    options::{IndexOptions, ReplaceOptions},
};
use serde::{Deserialize, Serialize};

use crate::config_type::MongoDBConfigure;

#[derive(Debug, Serialize, Deserialize)]
pub struct PostDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    #[serde(rename = "id")]
    pub post_id: u64,
    pub from: u64,
    pub data: Document,
    pub fetched_at: DateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReplyDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub rpid: u64,
    pub oid: u64,
    #[serde(rename = "oidType")]
    pub oid_type: u64,
    pub ctime: u64,
    pub uid: u64,
    pub parent: u64,
    pub nickname: String,
    pub content: String,
    pub like: u64,
    #[serde(rename = "replyControl")]
    pub reply_control: Document,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<DateTime>,
}

pub struct ResultDb {
    post_collection: Collection<PostDocument>,
    reply_collection: Collection<ReplyDocument>,
}

impl ResultDb {
    pub async fn new(config: &MongoDBConfigure) -> Self {
        let uri = &config.uri.as_str();
        let db_name = &config.database.as_str();
        let post_collection_name = &config.collections.posts.as_str();
        let reply_collection_name = &&config.collections.replies.as_str();
        let client = Client::with_uri_str(uri).await.unwrap();
        let database = client.database(db_name);
        let post_collection = database.collection(post_collection_name);
        let reply_collection = database.collection(reply_collection_name);
        let db = Self {
            post_collection,
            reply_collection,
        };
        db.create_indexes().await;
        db
    }

    async fn create_indexes(&self) {
        let post_id_index = IndexModel::builder()
            .keys(doc! { "id": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();
        let reply_rpid_index = IndexModel::builder()
            .keys(doc! { "rpid": 1 })
            .options(IndexOptions::builder().unique(true).build())
            .build();
        let reply_oid_index = IndexModel::builder().keys(doc! { "oid": 1 }).build();

        self.post_collection
            .create_index(post_id_index)
            .await
            .unwrap();
        self.reply_collection
            .create_index(reply_rpid_index)
            .await
            .unwrap();
        self.reply_collection
            .create_index(reply_oid_index)
            .await
            .unwrap();
    }

    pub async fn save_post(&self, id: &u64, from: &u64, data: Document) {
        let post = PostDocument {
            id: None,
            post_id: id.clone(),
            from: from.clone(),
            data,
            fetched_at: DateTime::now(),
        };
        let options = ReplaceOptions::builder().upsert(true).build();
        self.post_collection
            .replace_one(doc! { "id": *id as i64 }, post)
            .with_options(options)
            .await
            .unwrap();
    }

    pub async fn get_all_posts_cursor(&self) -> Cursor<PostDocument> {
        self.post_collection.find(doc! {}).await.unwrap()
    }

    pub async fn get_post_by_id(&self, id: u64) -> Option<PostDocument> {
        self.post_collection
            .find(doc! { "id": id as i64 })
            .await
            .unwrap()
            .try_next()
            .await
            .unwrap()
    }

    pub async fn save_reply(&self, reply: ReplyDocument) -> Result<(), mongodb::error::Error> {
        let reply = ReplyDocument {
            fetched_at: Some(DateTime::now()),
            ..reply
        };
        self.reply_collection.insert_one(reply).await?;
        Ok(())
    }
}
