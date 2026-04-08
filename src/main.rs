// 说起来，我已经写了这么多unwarp了
mod config_type;
mod db;
mod fetch_posts;
mod fetch_reply;
mod open_page;
mod post_parser;
mod utils;

use std::{
    fs::File,
    io::{Read, Write},
    process::exit,
    thread,
    time::Duration,
};

use clap::Command;
use fetch_reply::FetchMode;

use futures_util::{StreamExt, TryStreamExt};
use open_page::inject_functions;

use crate::{
    config_type::Configure,
    db::{result_db::ResultDb, runtime_db::RuntimeDb},
    post_parser::{DynamicType, parse_dynamic_item},
    utils::wait_until_enter,
};

#[tokio::main]
async fn main() {
    let cmd = Command::new("B站动态和回复爬虫")
        .about("通过Chrome爬B站的动态和对应的的评论区")
        .author("xys20071111")
        .subcommand_required(true)
        .subcommand(
            Command::new("login")
                .about("登录B站账号")
                .arg(clap::arg!(-c --config <FILE> "配置文件").required(true))
                .arg(clap::arg!(-d --debug "开启devtool")),
        )
        .subcommand(
            Command::new("post")
                .about("获取动态")
                .arg(clap::arg!(-c --config <FILE> "配置文件").required(true))
                .arg(clap::arg!(-d --debug "开启devtool"))
                .arg(clap::arg!(--resume "从上次停止的位置继续（默认）"))
                .arg(clap::arg!(--refresh "从头开始重新获取")),
        )
        .subcommand(
            Command::new("reply")
                .about("获取评论")
                .arg(clap::arg!(-c --config <FILE> "配置文件").required(true))
                .arg(clap::arg!(-d --debug "开启devtool"))
                .arg(clap::arg!(--resume "从上次停止的页码继续（默认）"))
                .arg(clap::arg!(--refresh "从第一页重新开始")),
        )
        .subcommand(
            Command::new("export-post")
                .about("将已储存的动态导出为jsonl格式")
                .arg(clap::arg!(-r --raw "导出为原始格式"))
                .arg(clap::arg!(-c --config <FILE> "配置文件").required(true))
                .arg(clap::arg!(<PATH> "保存路径").required(true)),
        );

    let matches = cmd.try_get_matches().unwrap_or_else(|e| e.exit());

    match matches.subcommand() {
        Some((command_name, sub_matches)) => match command_name {
            "login" => {
                let config_path = sub_matches.get_one::<String>("config").unwrap();
                let debug = sub_matches.get_flag("debug");
                handle_login_mode(config_path, debug).await;
            }
            "post" => {
                let config_path = sub_matches.get_one::<String>("config").unwrap();
                let debug = sub_matches.get_flag("debug");
                let mode = get_fetch_mode(
                    sub_matches.get_flag("resume"),
                    sub_matches.get_flag("refresh"),
                );
                handle_post_mode(config_path, debug, mode).await;
            }
            "reply" => {
                let config_path = sub_matches.get_one::<String>("config").unwrap();
                let debug = sub_matches.get_flag("debug");
                let mode = get_fetch_mode(
                    sub_matches.get_flag("resume"),
                    sub_matches.get_flag("refresh"),
                );
                handle_reply_mode(config_path, debug, mode).await;
            }
            "export-post" => {
                let config_path = sub_matches.get_one::<String>("config").unwrap();
                let raw = sub_matches.get_flag("raw");
                let path = sub_matches.get_one::<String>("PATH").unwrap();
                handle_export_post(config_path, path.as_str(), raw).await;
            }
            _ => unreachable!(),
        },
        None => {
            eprintln!("需要指定子命令");
            exit(1);
        }
    }
}

fn get_fetch_mode(_resume: bool, refresh: bool) -> FetchMode {
    if refresh {
        FetchMode::Refresh
    } else {
        FetchMode::Resume
    }
}

async fn handle_login_mode(config_path: &str, _debug: bool) {
    let config = load_config(config_path);
    let browser = open_page::open_browser(false, false, &config.browser_data_path).unwrap();
    let tab = browser.new_tab().unwrap();
    tab.navigate_to("https://www.bilibili.com").unwrap();
    wait_until_enter();
    exit(0)
}

async fn handle_export_post(config_path: &str, output_file_path: &str, raw: bool) {
    let config = load_config(config_path);
    let result_db = ResultDb::new(&config.mongodb).await;
    let mut posts = result_db.get_all_posts_cursor().await;
    let mut output_file = File::create_new(output_file_path).unwrap();
    while posts.has_next() {
        if raw {
            let post = posts.next().await.unwrap().unwrap().data;
            let post = serde_json::to_string(&post).unwrap();
            output_file.write((post + "\n").as_bytes()).unwrap();
        } else {
            let post = posts.next().await.unwrap().unwrap().data;
            let post = parse_dynamic_item(&serde_json::to_value(post).unwrap());
            let post = serde_json::to_string(&post).unwrap();
            output_file.write((post + "\n").as_bytes()).unwrap();
        }
    }
}

async fn handle_post_mode(config_path: &str, debug: bool, mode: FetchMode) {
    let config = load_config(config_path);
    let browser =
        open_page::open_browser(config.headless, debug, &config.browser_data_path.as_str())
            .unwrap();
    let tab = browser.new_tab().unwrap();
    tab.navigate_to("https://www.bilibili.com").unwrap();
    inject_functions(&tab);

    let result_db = ResultDb::new(&config.mongodb).await;
    let runtime_db = RuntimeDb::new(&config.runtime_db_name);

    match mode {
        FetchMode::Resume => {
            for item in &config.sources {
                let last_fetch_time = runtime_db.get_source_last_fetch(item.id);
                fetch_posts::fetch_post_ids_from_browser(
                    &tab,
                    &item,
                    &last_fetch_time,
                    &String::new(),
                    &runtime_db,
                );
                println!("[{}] 获取成功，休眠 5 秒...", item.name);
                thread::sleep(Duration::from_secs(5));
            }
        }
        FetchMode::Refresh => {
            for item in &config.sources {
                fetch_posts::fetch_post_ids_from_browser(
                    &tab,
                    &item,
                    &0,
                    &String::new(),
                    &runtime_db,
                );
                println!("[{}] 获取成功，休眠 5 秒...", item.name);
                thread::sleep(Duration::from_secs(5));
            }
        }
    }

    let pending_posts = runtime_db.get_pending_posts();
    fetch_posts::fetch_post_details_from_browser(&tab, &result_db, &runtime_db, &pending_posts)
        .await;

    let posts = result_db.get_all_posts().await;
    for item in posts {
        let data_value = serde_json::to_value(&item.data).unwrap();
        let parsed = parse_dynamic_item(&data_value);
        if let DynamicType::Forward = parsed.dynamic_type
            && let Some(id) = parsed.original_post_id
        {
            if result_db.get_post_by_id(id).await.is_none() {
                runtime_db.add_post_to_queue(id, "原始动态");
            }
        }
    }
    let pending_posts = runtime_db.get_pending_posts();
    fetch_posts::fetch_post_details_from_browser(&tab, &result_db, &runtime_db, &pending_posts)
        .await;

    if debug {
        println!("按回车键结束...");
        wait_until_enter();
    }
}

async fn handle_reply_mode(config_path: &str, debug: bool, mode: FetchMode) {
    let config = load_config(config_path);
    let browser =
        open_page::open_browser(config.headless, debug, &config.browser_data_path.as_str())
            .unwrap();
    let tab = browser.new_tab().unwrap();
    tab.navigate_to("https://www.bilibili.com").unwrap();
    inject_functions(&tab);

    let result_db = ResultDb::new(&config.mongodb).await;
    let runtime_db = RuntimeDb::new(&config.runtime_db_name);

    fetch_reply::fetch_replies_from_browser(&tab, &result_db, &runtime_db, &config, mode).await;

    if debug {
        println!("按回车键结束...");
        wait_until_enter();
    }
}

fn load_config(config_path: &str) -> Configure {
    let mut config_file = File::open(config_path).unwrap();
    let mut raw_config_json = String::new();
    config_file.read_to_string(&mut raw_config_json).unwrap();
    serde_json::from_str(&raw_config_json).unwrap()
}
