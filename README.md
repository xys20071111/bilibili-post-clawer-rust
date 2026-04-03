### 自己写的话

以前用ts写的爬虫用rust搞了一遍，除了db和fetch_reply基本都是自己写的，不过main被ai大修了一遍，几乎看不出多少我自己写的痕迹了。  
post_parser本来就是AI生成的，直接用AI移植了。  
也许应该直接整个用AI来移植才是，那样移植速度肯定会快很多

### 以下为AI生成的README

# B站动态爬虫 (Rust 版)

通过无头 Chrome 爬取 B 站的动态和对应的评论区数据。

本项目是原 Deno 版本的 Rust 重写版，提供更高效的性能和更便捷的使用方式。

## 环境要求

- Rust 1.75+ (版本不限，最新稳定版即可)
- Chrome 或 Chromium 浏览器
- MongoDB

## 功能特性

- 支持多个数据源（UID）的动态爬取
- 支持动态详情和评论区爬取
- 支持断点续爬
- 支持转发动态的原始动态获取
- 使用 MongoDB 存储结果数据
- 使用 SQLite 存储运行时状态

## 使用说明

### 1. 登录 B 站

```bash
cargo run -- login -c config.json
```

打开浏览器后登录 B 站账号，登录完成后关闭浏览器。

### 2. 获取动态

```bash
# 从头开始获取
cargo run -- post -c config.json --refresh

# 从上次停止位置继续（默认）
cargo run -- post -c config.json --resume
```

### 3. 获取评论

```bash
# 从头开始获取
cargo run -- reply -c config.json --refresh

# 从上次停止位置继续（默认）
cargo run -- reply -c config.json --resume
```

### 4. 调试模式

添加 `-d` 参数可开启 Chrome DevTools 并在结束后等待用户输入：

```bash
cargo run -- post -c config.json -d
```

## 配置说明

创建 `config.json` 配置文件：

```json
{
  "browserDataPath": "/path/to/chrome/user/data",
  "headless": true,
  "runtimeDbName": "runtime.sqlite3",
  "mongodb": {
    "uri": "mongodb://localhost:27017",
    "database": "bilibili",
    "collections": {
      "posts": "posts",
      "replies": "replies"
    }
  },
  "sources": [
    {
      "name": "用户名 1",
      "id": "12345678"
    },
    {
      "name": "用户名 2",
      "id": "87654321"
    }
  ]
}
```

### 配置项说明

| 配置项 | 类型 | 说明 |
|--------|------|------|
| `browserDataPath` | string | Chrome 浏览器数据目录路径（用于保存登录状态） |
| `headless` | boolean | 是否启用无头模式 |
| `runtimeDbName` | string | SQLite 数据库文件名（存储运行时状态） |
| `mongodb.uri` | string | MongoDB 连接 URI |
| `mongodb.database` | string | MongoDB 数据库名称 |
| `mongodb.collections.posts` | string | 存储动态详情的集合名 |
| `mongodb.collections.replies` | string | 存储评论的集合名 |
| `sources` | array | 数据源列表 |
| `sources[].name` | string | 用户昵称（用于显示） |
| `sources[].id` | string | B 站用户 UID |

## 数据存储说明

### SQLite (运行时数据库)

- 待获取的动态 ID 列表
- 各目标的最后爬取时间
- 各动态评论区的爬取进度

### MongoDB (结果数据库)

- `posts` 集合：动态详情数据
- `replies` 集合：评论数据

## 项目结构

```
.
├── src/
│   ├── main.rs          # 主程序入口
│   ├── config_type.rs   # 配置类型定义
│   ├── open_page.rs     # Chrome 浏览器控制
│   ├── fetch_posts.rs   # 动态爬取逻辑
│   ├── fetch_reply.rs   # 评论爬取逻辑
│   ├── post_parser.rs   # 动态数据解析
│   ├── db/
│   │   ├── mod.rs       # 数据库模块
│   │   ├── result_db.rs # MongoDB 结果数据库
│   │   └── runtime_db.rs# SQLite 运行时数据库
│   └── utils.rs         # 工具函数
├── configs/             # 配置文件目录
├── dbs/                 # SQLite 数据库目录
└── Cargo.toml           # Rust 项目配置
```

## 构建和运行

```bash
# 构建
cargo build --release

# 运行
cargo run -- <command> [options]
```

## 依赖

- [clap](https://crates.io/crates/clap) - 命令行参数解析
- [headless_chrome](https://crates.io/crates/headless_chrome) - Chrome 无头浏览器控制
- [tokio](https://crates.io/crates/tokio) - 异步运行时
- [mongodb](https://crates.io/crates/mongodb) - MongoDB 驱动
- [rusqlite](https://crates.io/crates/rusqlite) - SQLite 驱动
- [serde](https://crates.io/crates/serde) - 序列化/反序列化

## 注意事项

- 谨慎使用，小心IP被Ban导致你看不了B站
- 本项目仅供学习研究使用

## License

MIT License
