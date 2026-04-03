use rusqlite::{Connection, params};

const SQL_SCHEME: &str = r#"
-- Post queue 表：待获取的动态ID队列（替代 DenoKV ["postId", id]）
CREATE TABLE IF NOT EXISTS post_queue (
    post_id INTEGER PRIMARY KEY,
    source TEXT NOT NULL,
    added_at INTEGER NOT NULL
);

-- Source last fetch 表：记录每个来源的最后获取时间（替代 DenoKV ["lastFetchDate", sourceId]）
CREATE TABLE IF NOT EXISTS source_last_fetch (
    source_id INTEGER PRIMARY KEY,
    last_fetch_time INTEGER NOT NULL
);

-- Reply fetch progress 表：记录评论获取进度（替代 DenoKV ["reply_page", oid]）
CREATE TABLE IF NOT EXISTS reply_fetch_progress (
    oid INTEGER PRIMARY KEY,
    page_num INTEGER NOT NULL DEFAULT 1,
    last_fetched_at INTEGER,
    blocked INTEGER DEFAULT 0
);
"#;

pub struct PendingPost {
    pub post_id: u64,
    pub source: String,
}

pub struct ReplyProgress {
    pub oid: u64,
    pub page_num: u64,
    pub last_fetched_at: Option<u64>,
    pub blocked: bool,
}

pub struct RuntimeDb {
    conn: Connection,
}

impl RuntimeDb {
    pub fn new(path: &str) -> Self {
        let db = Self {
            conn: Connection::open(path).unwrap(),
        };
        db.conn.execute_batch(SQL_SCHEME).unwrap();
        db
    }

    pub fn add_post_to_queue(&self, post_id: u64, source: &str) {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO post_queue (post_id, source, added_at) VALUES (?1, ?2, ?3)",
                params![
                    post_id,
                    source,
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                ],
            )
            .unwrap();
    }

    pub fn remove_post_from_queue(&self, post_id: u64) {
        self.conn
            .execute(
                "DELETE FROM post_queue WHERE post_id = ?1",
                params![post_id],
            )
            .unwrap();
    }

    pub fn get_pending_posts(&self) -> Vec<PendingPost> {
        let mut stmt = self
            .conn
            .prepare("SELECT post_id, source FROM post_queue")
            .unwrap();
        stmt.query_map([], |row| {
            Ok(PendingPost {
                post_id: row.get(0)?,
                source: row.get(1)?,
            })
        })
        .unwrap()
        .into_iter()
        .map(|r| r.unwrap())
        .collect()
    }

    pub fn set_source_last_fetch(&self, source_id: u64) {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO source_last_fetch (source_id, last_fetch_time) VALUES (?1, ?2)",
                params![
                    source_id,
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                ],
            )
            .unwrap();
    }
    pub fn get_source_last_fetch(&self, source_id: u64) -> u64 {
        self.conn
            .query_row(
                "SELECT * FROM source_last_fetch WHERE source_id = ?1",
                params![source_id],
                |row| Ok(row.get(1).unwrap()),
            )
            .unwrap_or(0)
    }

    pub fn get_reply_progress(&self, oid: u64) -> Option<ReplyProgress> {
        self.conn
            .query_row(
                "SELECT oid, page_num, last_fetched_at, blocked FROM reply_fetch_progress WHERE oid = ?1",
                params![oid],
                |row| {
                    Ok(ReplyProgress {
                        oid: row.get(0)?,
                        page_num: row.get(1)?,
                        last_fetched_at: row.get(2)?,
                        blocked: row.get::<_, i32>(3)? != 0,
                    })
                },
            )
            .ok()
    }

    pub fn set_reply_progress(&self, oid: u64, page_num: u64, blocked: bool) {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO reply_fetch_progress (oid, page_num, last_fetched_at, blocked) VALUES (?1, ?2, NULL, ?3)",
                params![oid, page_num, blocked as i32],
            )
            .unwrap();
    }

    pub fn set_reply_last_fetched(&self, oid: u64, page_num: u64) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.conn
            .execute(
                "INSERT OR REPLACE INTO reply_fetch_progress (oid, page_num, last_fetched_at, blocked) VALUES (?1, ?2, ?3, 0)",
                params![oid, page_num, now],
            )
            .unwrap();
    }
}
