use core::panic;
use headless_chrome::Tab;
use serde_json::{Value, json};
use std::{
    thread,
    time::{self, Duration},
};

use crate::{
    config_type::SourceStruct,
    db::{
        result_db::ResultDb,
        runtime_db::{PendingPost, RuntimeDb},
    },
    post_parser::parse_dynamic_item,
    utils::wait_until_enter,
};

pub fn fetch_post_ids_from_browser(
    tab: &Tab,
    source: &SourceStruct,
    stop_at: &u64,
    base_offset: &String,
    rumtime_db_instance: &RuntimeDb,
) {
    let mut has_more = true;
    let mut current_offset = base_offset.clone();
    while has_more {
        println!("[{}] 当前偏移：{}", source.name, current_offset);
        let exec_code = r#"
        (async () => {
            async function fetchPostIds() {
                const { mid, offset } = {{missionInfo}}
                const req = await fetch(
                    `https://api.bilibili.com/x/polymer/web-dynamic/v1/feed/space?offset=${offset}&host_mid=${mid}&timezone_offset=-480`,
                    {
                        credentials: "include",
                    },
                )
                if (!req.ok) {
                    await denoAlert(
                        `request failed! Is your ip banned? currentOffset: ${offset} Code: ${req.status}`,
                    )
                    return null
                }
                const res = await req.text()
                return res
            }
            return await fetchPostIds()
        })()"#
        .replace("{{missionInfo}}", serde_json::to_string(&json!({ "mid": source.id, "offset": current_offset })).unwrap().as_str());
        for _i in 0..5 {
            let result: Value = serde_json::from_str(
                tab.evaluate(&exec_code, true)
                    .unwrap()
                    .value
                    .unwrap()
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
            if result["code"] != 0 {
                eprintln!(
                    "请求失败！错误码: {}. 是不是ip被ban了？",
                    result["code"].as_i64().unwrap()
                );
                wait_until_enter();
                continue;
            } else {
                let data = &result["data"];
                has_more = data["has_more"].as_bool().unwrap();
                current_offset = String::from(data["offset"].as_str().unwrap());
                for item in data["items"].as_array().unwrap() {
                    let parsed_post = parse_dynamic_item(item);
                    let id = parsed_post.id;
                    let publish_time = parsed_post.publish_time.unwrap_or(0);
                    if publish_time < stop_at.clone() {
                        println!("动态 {} 发布时间早于 {}，停止获取！", id, stop_at);
                        return;
                    }
                    rumtime_db_instance.add_post_to_queue(id, &source.name);
                }
                break;
            }
        }
        thread::sleep(time::Duration::from_secs(3));
    }
    rumtime_db_instance.set_source_last_fetch(source.id);
}

pub async fn fetch_post_details_from_browser(
    tab: &Tab,
    result_db: &ResultDb,
    runtime_db: &RuntimeDb,
    post_info: &Vec<PendingPost>,
) {
    for post in post_info {
        let exec_code = r#"
(async () => {
async function fetchPostDetails() {
  const id = '{{id}}'
  const req = await fetch(
    `https://api.bilibili.com/x/polymer/web-dynamic/v1/detail?id=${id}&features=itemOpusStyle,opusBigCover,onlyfansVote,endFooterHidden,decorationCard,onlyfansAssetsV2,ugcDelete,onlyfansQaCard,editable,opusPrivateVisible,avatarAutoTheme,sunflowerStyle,cardsEnhance,eva3CardOpus,eva3CardVideo,eva3CardComment,eva3CardVote,eva3CardUser`,
    {
      credentials: 'include',
    },
  )
  if (req.status === 412) {
    await denoAlert(
      `request failed! Is your ip banned? currentId: ${id} Code: ${req.status}`,
    )
    return JSON.stringify({ code: 412 })
  }
  const res = await req.text()
  return res
}
return await fetchPostDetails();
})()
        "#.replace("{{id}}", post.post_id.to_string().as_str());
        let mut succeed_flag = false;
        for i in 0..5 {
            println!("正在获取 {} 发布的动态 {}", post.source, post.post_id);
            let result = tab.evaluate(exec_code.as_str(), true).unwrap();
            let result = match result.value {
                Some(val) => val,
                _ => {
                    eprintln!("出现异常!!!");
                    eprintln!("{:?}", result);
                    eprintln!("exec_code:\n{}", exec_code);
                    wait_until_enter();
                    panic!("出现空白响应")
                }
            };
            let result: Value = serde_json::from_str(result.as_str().unwrap()).unwrap();
            if result["code"] != 0 {
                let error_code = result["code"].as_i64().unwrap();
                match error_code {
                    500 => {
                        println!("{} 需要重试", post.post_id);
                    }
                    -1024 => {
                        println!("{} 不存在，跳过", post.post_id);
                        runtime_db.remove_post_from_queue(post.post_id);
                        succeed_flag = true;
                        break;
                    }
                    4101152 => {
                        println!("{} 不存在，跳过", post.post_id);
                        runtime_db.remove_post_from_queue(post.post_id);
                        succeed_flag = true;
                        break;
                    }
                    _ => {
                        eprintln!("请求出错，错误码: {}. 是不是需要人机验证了?", error_code);
                        wait_until_enter();
                    }
                }
            } else {
                let parsed_post = parse_dynamic_item(&result["data"]["item"]);
                let author = parsed_post.author.unwrap();
                result_db
                    .save_post(
                        &post.post_id,
                        &author.mid,
                        bson::to_document(&result["data"]["item"]).unwrap(),
                    )
                    .await;
                runtime_db.remove_post_from_queue(post.post_id);
                println!("已获取 {} 发布的动态 {}", &author.name, post.post_id);
                succeed_flag = true;
                break;
            }
            eprintln!("重试中，次数 {}", i);
        }
        if !succeed_flag {
            eprintln!("动态 {} 获取失败", post.post_id)
        }
        thread::sleep(Duration::from_secs(3));
    }
}
