use chrono::Utc;
use serde_json::{Value, json};
use tokio_postgres::Client;

use crate::config_type::PostgreSQLConfigure;

pub struct PostgresDb {
    client: Client,
}

impl PostgresDb {
    pub async fn new(config: &PostgreSQLConfigure) -> Self {
        let conn_str = format!(
            "host={} port={} dbname={} user={} password={}",
            config.host, config.port, config.database, config.username, config.password
        );
        let (client, connection) = tokio_postgres::connect(&conn_str, tokio_postgres::NoTls)
            .await
            .unwrap();

        // Spawn connection driver
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("PostgreSQL connection error: {}", e);
            }
        });

        let db = Self { client };
        db.create_tables().await;
        db
    }

    async fn create_tables(&self) {
        self.client
            .batch_execute(
                "
                CREATE TABLE IF NOT EXISTS posts (
                    post_id BIGINT PRIMARY KEY,
                    from_uid BIGINT NOT NULL,
                    data JSONB NOT NULL,
                    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE TABLE IF NOT EXISTS replies (
                    rpid BIGINT PRIMARY KEY,
                    oid BIGINT NOT NULL,
                    oid_type BIGINT NOT NULL,
                    ctime BIGINT NOT NULL,
                    uid BIGINT NOT NULL,
                    parent BIGINT NOT NULL,
                    nickname TEXT NOT NULL,
                    content TEXT NOT NULL,
                    like_count BIGINT NOT NULL,
                    reply_control JSONB NOT NULL DEFAULT '{}',
                    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS idx_replies_oid ON replies (oid);
                ",
            )
            .await
            .unwrap();
    }

    pub async fn save_post(&self, id: &u64, from: &u64, data: serde_json::Value) {
        self.client
            .execute(
                "INSERT INTO posts (post_id, from_uid, data, fetched_at) VALUES ($1, $2, $3, $4)
                 ON CONFLICT (post_id) DO UPDATE SET from_uid = $2, data = $3, fetched_at = $4",
                &[
                    &(*id as i64),
                    &(*from as i64),
                    &data,
                    &Utc::now(),
                ],
            )
            .await
            .unwrap();
    }

    pub async fn get_all_posts(&self) -> Vec<serde_json::Value> {
        let rows = self
            .client
            .query("SELECT post_id, from_uid, data, fetched_at FROM posts", &[])
            .await
            .unwrap();

        rows.iter()
            .map(|row| {
                let post_id: i64 = row.get(0);
                let from_uid: i64 = row.get(1);
                let data: Value = row.get(2);
                let fetched_at: chrono::DateTime<Utc> = row.get(3);
                json!({
                    "id": post_id as u64,
                    "from": from_uid as u64,
                    "data": data,
                    "fetched_at": { "$date": fetched_at.timestamp_millis() }
                })
            })
            .collect()
    }

    pub async fn get_post_by_id(&self, id: u64) -> Option<serde_json::Value> {
        let row = self
            .client
            .query_opt(
                "SELECT post_id, from_uid, data, fetched_at FROM posts WHERE post_id = $1",
                &[&(id as i64)],
            )
            .await
            .unwrap();

        row.map(|row| {
            let post_id: i64 = row.get(0);
            let from_uid: i64 = row.get(1);
            let data: Value = row.get(2);
            let fetched_at: chrono::DateTime<Utc> = row.get(3);
            json!({
                "id": post_id as u64,
                "from": from_uid as u64,
                "data": data,
                "fetched_at": { "$date": fetched_at.timestamp_millis() }
            })
        })
    }

    pub async fn save_reply(
        &self,
        rpid: u64,
        oid: u64,
        oid_type: u64,
        ctime: u64,
        uid: u64,
        parent: u64,
        nickname: &str,
        content: &str,
        like: u64,
        reply_control: Value,
    ) {
        self.client
            .execute(
                "INSERT INTO replies (rpid, oid, oid_type, ctime, uid, parent, nickname, content, like_count, reply_control, fetched_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                &[
                    &(rpid as i64),
                    &(oid as i64),
                    &(oid_type as i64),
                    &(ctime as i64),
                    &(uid as i64),
                    &(parent as i64),
                    &nickname,
                    &content,
                    &(like as i64),
                    &reply_control,
                    &Utc::now(),
                ],
            )
            .await
            .unwrap();
    }
}
