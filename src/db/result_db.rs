use bson::{DateTime, Document, doc, oid::ObjectId};
use serde::{Deserialize, Serialize};

use mongodb::{
    Client, Collection, IndexModel,
    options::{IndexOptions, ReplaceOptions},
};
use futures_util::TryStreamExt;

use crate::config_type::Configure;
use crate::db::postgres_db::PostgresDb;

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

pub enum ResultDb {
    MongoDB {
        post_collection: Collection<PostDocument>,
        reply_collection: Collection<ReplyDocument>,
    },
    PostgreSQL(PostgresDb),
}

impl ResultDb {
    pub async fn new(config: &Configure) -> Self {
        if let Some(pg_config) = &config.postgresql {
            ResultDb::PostgreSQL(PostgresDb::new(pg_config).await)
        } else if let Some(mongo_config) = &config.mongodb {
            let uri = mongo_config.uri.as_str();
            let db_name = mongo_config.database.as_str();
            let post_collection_name = mongo_config.collections.posts.as_str();
            let reply_collection_name = mongo_config.collections.replies.as_str();
            let client = Client::with_uri_str(uri).await.unwrap();
            let database = client.database(db_name);
            let post_collection = database.collection(post_collection_name);
            let reply_collection = database.collection(reply_collection_name);
            let db = ResultDb::MongoDB {
                post_collection,
                reply_collection,
            };
            db.create_indexes().await;
            db
        } else {
            panic!("配置中必须指定 mongodb 或 postgresql 至少一个");
        }
    }

    async fn create_indexes(&self) {
        if let ResultDb::MongoDB {
            post_collection,
            reply_collection,
        } = self
        {
            let post_id_index = IndexModel::builder()
                .keys(doc! { "id": 1 })
                .options(IndexOptions::builder().unique(true).build())
                .build();
            let reply_rpid_index = IndexModel::builder()
                .keys(doc! { "rpid": 1 })
                .options(IndexOptions::builder().unique(true).build())
                .build();
            let reply_oid_index = IndexModel::builder().keys(doc! { "oid": 1 }).build();

            post_collection
                .create_index(post_id_index)
                .await
                .unwrap();
            reply_collection
                .create_index(reply_rpid_index)
                .await
                .unwrap();
            reply_collection
                .create_index(reply_oid_index)
                .await
                .unwrap();
        }
    }

    pub async fn save_post(&self, id: &u64, from: &u64, data: Document) {
        match self {
            ResultDb::MongoDB {
                post_collection, ..
            } => {
                let post = PostDocument {
                    id: None,
                    post_id: *id,
                    from: *from,
                    data,
                    fetched_at: DateTime::now(),
                };
                let options = ReplaceOptions::builder().upsert(true).build();
                post_collection
                    .replace_one(doc! { "id": *id as i64 }, post)
                    .with_options(options)
                    .await
                    .unwrap();
            }
            ResultDb::PostgreSQL(pg) => {
                let data_json = serde_json::to_value(&data).unwrap();
                pg.save_post(id, from, data_json).await;
            }
        }
    }

    pub async fn get_all_posts_cursor(&self) -> Vec<PostDocument> {
        match self {
            ResultDb::MongoDB {
                post_collection, ..
            } => {
                let cursor = post_collection.find(doc! {}).await.unwrap();
                cursor.try_collect().await.unwrap()
            }
            ResultDb::PostgreSQL(pg) => {
                let json_docs = pg.get_all_posts().await;
                json_docs
                    .into_iter()
                    .map(|v| {
                        let data: Document =
                            serde_json::from_value(v["data"].clone()).unwrap();
                        let fetched_at_millis = v["fetched_at"]["$date"].as_i64().unwrap();
                        PostDocument {
                            id: None,
                            post_id: v["id"].as_u64().unwrap(),
                            from: v["from"].as_u64().unwrap(),
                            data,
                            fetched_at: DateTime::from_millis(fetched_at_millis),
                        }
                    })
                    .collect()
            }
        }
    }

    pub async fn get_post_by_id(&self, id: u64) -> Option<PostDocument> {
        match self {
            ResultDb::MongoDB {
                post_collection, ..
            } => {
                use futures_util::StreamExt;
                post_collection
                    .find(doc! { "id": id as i64 })
                    .await
                    .unwrap()
                    .next()
                    .await
                    .transpose()
                    .unwrap()
            }
            ResultDb::PostgreSQL(pg) => {
                let v = pg.get_post_by_id(id).await?;
                let data: Document = serde_json::from_value(v["data"].clone()).unwrap();
                let fetched_at_millis = v["fetched_at"]["$date"].as_i64().unwrap();
                Some(PostDocument {
                    id: None,
                    post_id: v["id"].as_u64().unwrap(),
                    from: v["from"].as_u64().unwrap(),
                    data,
                    fetched_at: DateTime::from_millis(fetched_at_millis),
                })
            }
        }
    }

    pub async fn save_reply(&self, reply: ReplyDocument) -> Result<(), String> {
        match self {
            ResultDb::MongoDB {
                reply_collection, ..
            } => {
                let reply = ReplyDocument {
                    fetched_at: Some(DateTime::now()),
                    ..reply
                };
                reply_collection
                    .insert_one(reply)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
            ResultDb::PostgreSQL(pg) => {
                let reply_control_json = serde_json::to_value(&reply.reply_control).unwrap();
                pg.save_reply(
                    reply.rpid,
                    reply.oid,
                    reply.oid_type,
                    reply.ctime,
                    reply.uid,
                    reply.parent,
                    &reply.nickname,
                    &reply.content,
                    reply.like,
                    reply_control_json,
                )
                .await;
                Ok(())
            }
        }
    }
}
