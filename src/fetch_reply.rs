use std::{thread, time, time::Duration};

use headless_chrome::Tab;
use serde_json::Value;

use crate::{
    config_type::Configure,
    db::{
        result_db::{ReplyDocument, ResultDb},
        runtime_db::RuntimeDb,
    },
    post_parser::parse_dynamic_item,
    utils::wait_until_enter,
    wbi_sign::WbiSign,
};

#[derive(Debug, Clone, Copy)]
pub enum FetchMode {
    Resume,
    Refresh,
}

pub async fn fetch_replies_from_browser(
    tab: &Tab,
    result_db: &ResultDb,
    runtime_db: &RuntimeDb,
    config: &Configure,
    mode: FetchMode,
) {
    let posts = result_db.get_all_posts_cursor().await;
    let mut post_ids: Vec<(u64, u64)> = Vec::new();
    for post in posts {
        let parsed = parse_dynamic_item(&serde_json::to_value(post.data).unwrap());
        if let Some(comment_id) = parsed.comment_area.comment_id {
            let comment_id_num = comment_id.parse::<u64>().ok().unwrap();
            let comment_type = parsed.comment_area.comment_type.unwrap();
            post_ids.push((comment_id_num, comment_type));
        }
    }
    // .iter()
    // .filter_map(|post| {
    //     let data_value = serde_json::to_value(&post.data).unwrap();
    //     let parsed = parse_dynamic_item(&data_value);
    //     if let Some(comment_id) = parsed.comment_area.comment_id {
    //         let comment_id_num = comment_id.parse::<u64>().ok()?;
    //         let comment_type = parsed.comment_area.comment_type?;
    //         Some((comment_id_num, comment_type))
    //     } else {
    //         None
    //     }
    // })
    // .collect();

    let total_task_count = post_ids.len();
    println!("总任务数：{}", total_task_count);

    for (i, (oid, type_val)) in post_ids.iter().enumerate() {
        println!(
            "进度：{}/{} {:.4}%",
            i + 1,
            total_task_count,
            ((i + 1) as f64 / total_task_count as f64) * 100.0
        );
        fetch_post_replies_from_browser(tab, result_db, runtime_db, config, *oid, *type_val, &mode)
            .await;
    }
}

async fn fetch_post_replies_from_browser(
    tab: &Tab,
    result_db: &ResultDb,
    runtime_db: &RuntimeDb,
    config: &Configure,
    oid: u64,
    type_val: u64,
    mode: &FetchMode,
) {
    if oid == 0 {
        println!("oid 为 0，跳过");
        return;
    }

    let progress = runtime_db.get_reply_progress(oid);

    if let FetchMode::Resume = mode {
        if let Some(ref p) = progress {
            if p.blocked {
                println!("动态 {} 已标记为无法获取更多评论，跳过", oid);
                return;
            }

            if let Some(_last_fetched_at) = p.last_fetched_at {
                println!("当前动态已完成总体获取，请通过追加模式进行更新");
                return;
                // if config.skip_recently_fetched_days > 0 {
                //     let cooldown_ms =
                //         config.skip_recently_fetched_days as u64 * 24 * 60 * 60 * 1000;
                //     let elapsed_ms = (std::time::SystemTime::now()
                //         .duration_since(std::time::UNIX_EPOCH)
                //         .unwrap()
                //         .as_millis() as u64)
                //         - (last_fetched_at * 1000);
                //     let elapsed_days = elapsed_ms / (24 * 60 * 60 * 1000);
                //     if elapsed_ms < cooldown_ms {
                //         println!(
                //             "动态 {} 在冷却期内（{}天前获取过），跳过",
                //             oid, elapsed_days
                //         );
                //         return;
                //     }
                // }
            }
        }
    }

    let mut page_num = match mode {
        FetchMode::Resume => progress.map(|p| p.page_num).unwrap_or(1),
        FetchMode::Refresh => 1,
    };

    let mut has_more = true;
    println!(
        "正在获取动态 {} 的评论，从第 {} 页开始（模式：{}）...",
        oid,
        page_num,
        match mode {
            FetchMode::Resume => "继续",
            FetchMode::Refresh => "重新",
        }
    );
    let wbi_sign = WbiSign::new().await;
    while has_more {
        let mut error_retry_count = 0;
        let mut success = false;

        while !success {
            let wbi_signed_params = wbi_sign.encode_wbi(vec![
                ("oid", oid.to_string()),
                ("type", type_val.to_string()),
                ("pn", page_num.to_string()),
            ]);
            let exec_code = r#"
            (async () => {
                async function fetchPostReplies() {
                    const url = 'https://api.bilibili.com/x/v2/reply?{{missionInfo}}'
                    const req = await fetch(url, {
                        credentials: "include",
                    })
                    if (req.status === 412) {
                        await denoAlert(
                            `Request failed, your ip was banned.`,
                        )
                        return null
                    }
                    const res = await req.text()
                    return res
                }
                return await fetchPostReplies()
            })()
            "#
            .replace("{{missionInfo}}", wbi_signed_params.as_str());

            let result = tab.evaluate(exec_code.as_str(), true).unwrap();
            let result = match result.value {
                Some(val) => val,
                None => {
                    eprintln!("出现空白响应，是不是网断了？休眠 3 秒后继续重试...");
                    thread::sleep(time::Duration::from_secs(3));
                    continue;
                }
            };
            let result: Value = serde_json::from_str(result.as_str().unwrap()).unwrap();

            if result.is_null() {
                println!("IP 被 ban，停止获取");
                return;
            }

            if result["code"] != 0 {
                let error_code = result["code"].as_i64().unwrap();
                match error_code {
                    12002 | 12061 => {
                        println!("动态 {} 没有评论区", oid);
                        has_more = false;
                        success = true;
                        break;
                    }
                    -404 => {
                        println!("动态 {} 没有评论或已开启精选，返回码为 -404", oid);
                        println!("{}", exec_code);
                        wait_until_enter();
                        has_more = false;
                        success = true;
                        break;
                    }
                    -400 => {
                        println!("无法获取动态 {} 的更多评论，结果可能不完整", oid);
                        has_more = false;
                        if let FetchMode::Resume = mode {
                            runtime_db.set_reply_progress(oid, page_num, true);
                        }
                        success = true;
                        break;
                    }
                    412 => {
                        println!("IP 被 ban，停止获取");
                        return;
                    }
                    _ => {
                        if error_retry_count >= 5 {
                            println!("重试次数耗尽，跳过当前页");
                            println!("错误码: {}", error_code);
                            success = true;
                            break;
                        }
                        eprintln!("请求失败！错误码: {}. 是不是ip被ban了？", error_code);
                        wait_until_enter();
                        error_retry_count += 1;
                        thread::sleep(Duration::from_secs_f64(1.5));
                        continue;
                    }
                }
            }

            let data = &result["data"];
            has_more = data.get("replies").and_then(|r| r.as_array()).is_some();

            if !has_more {
                println!("动态 {} 评论获取完成", oid);
                runtime_db.set_reply_last_fetched(oid, page_num);
                success = true;
                break;
            }

            let replies = data["replies"].as_array().unwrap();
            let mut duplicate_count = 0;
            let mut total_count = 0;

            for item in replies {
                total_count += 1;
                let rpid_str = item["rpid_str"].as_str().unwrap_or("0");
                let oid_str = item["oid_str"].as_str().unwrap_or("0");
                let rpid = rpid_str.parse::<u64>().unwrap_or(0);
                let oid_num = oid_str.parse::<u64>().unwrap_or(0);

                let reply = ReplyDocument {
                    id: None,
                    rpid,
                    oid: oid_num,
                    oid_type: item["type"].as_u64().unwrap_or(0),
                    ctime: item["ctime"].as_u64().unwrap_or(0),
                    uid: item["mid_str"]
                        .as_str()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0),
                    parent: item["parent_str"]
                        .as_str()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0),
                    nickname: item["member"]["uname"].as_str().unwrap_or("").to_string(),
                    content: item["content"]["message"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    like: item["like"].as_u64().unwrap_or(0),
                    reply_control: bson::to_document(&item["reply_control"])
                        .unwrap_or(bson::Document::new()),
                    fetched_at: None,
                };

                let result = result_db.save_reply(reply).await;
                if result.is_err() {
                    duplicate_count += 1;
                }
            }

            println!(
                "第 {} 页：共 {} 条评论，{} 条重复，成功保存 {} 条",
                page_num,
                total_count,
                duplicate_count,
                total_count - duplicate_count
            );

            if let FetchMode::Refresh = mode {
                if total_count > 0 {
                    let duplicate_rate = duplicate_count as f64 / total_count as f64;
                    if duplicate_rate >= 0.8 {
                        println!(
                            "检测到 {} {} 条重复评论（{:.1}%），已获取到上次位置",
                            duplicate_count,
                            total_count,
                            duplicate_rate * 100.0
                        );
                        has_more = false;
                        runtime_db.set_reply_last_fetched(oid, page_num);
                        success = true;
                        break;
                    }
                    println!(
                        "检测到 {} {} 条重复评论（{:.1}%），继续获取",
                        duplicate_count,
                        total_count,
                        duplicate_rate * 100.0
                    );
                }
            }

            runtime_db.set_reply_progress(oid, page_num, false);
            success = true;
        }

        thread::sleep(Duration::from_secs_f64(1.5));
        page_num += 1;
    }
}
