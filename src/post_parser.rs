use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DynamicType {
    Forward,
    Video,
    Image,
    Text,
    Other,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthorInfo {
    pub mid: u64,
    pub name: String,
    pub face: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    pub title: String,
    pub description: String,
    pub cover: String,
    pub jump_url: String,
    pub duration: String,
    pub avid: String,
    pub bvid: String,
    pub stats: Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReserveInfo {
    pub title: String,
    pub publish_time_text: String,
    pub total: u64,
    pub jump_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentArea {
    pub comment_id: Option<String>,
    pub comment_type: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedDynamicItem {
    pub id: u64,
    pub author: Option<AuthorInfo>,
    pub publish_time: Option<u64>,
    pub publish_time_text: Option<String>,
    #[serde(rename = "type")]
    pub dynamic_type: DynamicType,
    pub original_post_id: Option<u64>,
    pub video_info: Option<VideoInfo>,
    pub image_urls: Option<Vec<String>>,
    pub content: Option<String>,
    pub reserve_info: Option<ReserveInfo>,
    pub comment_area: CommentArea,
}

fn find_module<'a>(modules: &'a Value, module_type: &str) -> Option<&'a Value> {
    if let Some(arr) = modules.as_array() {
        for m in arr {
            if m["module_type"] == module_type {
                return Some(m);
            }
        }
    }
    None
}

fn extract_u64_from_str(value: &Value) -> Option<u64> {
    value.as_str().and_then(|s| s.parse::<u64>().ok())
}

pub fn parse_dynamic_item(item: &Value) -> ParsedDynamicItem {
    let id_str = item["id_str"].as_str().unwrap_or("0");
    let id: u64 = id_str.parse().unwrap_or(0);

    let is_new_version = item["modules"].is_array();

    let author_module: Option<&Value>;
    let dynamic_module: Option<&Value>;
    let desc_module: Option<&Value>;
    let comment_module: Option<&Value>;

    if is_new_version {
        author_module =
            find_module(&item["modules"], "MODULE_TYPE_AUTHOR").map(|m| &m["module_author"]);
        dynamic_module =
            find_module(&item["modules"], "MODULE_TYPE_DYNAMIC").map(|m| &m["module_dynamic"]);
        desc_module = find_module(&item["modules"], "MODULE_TYPE_DESC").map(|m| &m["module_desc"]);
        comment_module =
            find_module(&item["modules"], "MODULE_TYPE_STAT").map(|m| &m["module_stat"]);
    } else if item["modules"].is_object() {
        let modules = &item["modules"];
        author_module = modules.get("module_author");
        dynamic_module = modules.get("module_dynamic");
        desc_module = dynamic_module.and_then(|dm| dm.get("desc"));
        comment_module = modules.get("module_stat");
    } else {
        author_module = None;
        dynamic_module = None;
        desc_module = None;
        comment_module = None;
    }

    let comment_area = if let Some(cm) = comment_module {
        if cm["comment"]["comment_id"].is_string() {
            CommentArea {
                comment_id: cm["comment"]["comment_id"].as_str().map(|s| s.to_string()),
                comment_type: cm["comment"]["comment_type"].as_u64(),
            }
        } else {
            CommentArea {
                comment_id: item["basic"]["comment_id_str"]
                    .as_str()
                    .map(|s| s.to_string()),
                comment_type: item["basic"]["comment_type"].as_u64(),
            }
        }
    } else {
        CommentArea {
            comment_id: item["basic"]["comment_id_str"]
                .as_str()
                .map(|s| s.to_string()),
            comment_type: item["basic"]["comment_type"].as_u64(),
        }
    };

    let author: Option<AuthorInfo> = author_module.map(|am| {
        let user = if am["user"].is_object() {
            &am["user"]
        } else {
            am
        };
        AuthorInfo {
            mid: user["mid"].as_u64().unwrap_or(0),
            name: user["name"].as_str().unwrap_or("").to_string(),
            face: user["face"].as_str().unwrap_or("").to_string(),
        }
    });

    let publish_time = author_module.and_then(|am| am["pub_ts"].as_u64());
    let publish_time_text = author_module
        .and_then(|am| {
            am.get("pub_time")
                .or_else(|| am.get("pub_text"))
                .and_then(|v| v.as_str())
        })
        .map(|s| s.to_string());

    let item_type = item["type"].as_str().unwrap_or("");

    let (dynamic_type, original_post_id, video_info, image_urls, content) = if is_new_version {
        parse_new_version(item_type, dynamic_module, desc_module, item)
    } else {
        parse_old_version(item_type, dynamic_module, desc_module, item)
    };

    let reserve_info = dynamic_module
        .and_then(|dm| dm.get("additional"))
        .filter(|add| add["type"] == "ADDITIONAL_TYPE_RESERVE")
        .and_then(|add| add.get("reserve"))
        .map(|reserve| ReserveInfo {
            title: reserve["title"].as_str().unwrap_or("").to_string(),
            publish_time_text: reserve["desc1"]["text"].as_str().unwrap_or("").to_string(),
            total: reserve["reserve_total"].as_u64().unwrap_or(0),
            jump_url: reserve["jump_url"].as_str().unwrap_or("").to_string(),
        });

    ParsedDynamicItem {
        id,
        author,
        publish_time,
        publish_time_text,
        dynamic_type,
        original_post_id,
        video_info,
        image_urls,
        content,
        reserve_info,
        comment_area,
    }
}

fn parse_new_version(
    item_type: &str,
    dynamic_module: Option<&Value>,
    desc_module: Option<&Value>,
    item: &Value,
) -> (
    DynamicType,
    Option<u64>,
    Option<VideoInfo>,
    Option<Vec<String>>,
    Option<String>,
) {
    match item_type {
        "DYNAMIC_TYPE_FORWARD" => {
            let content = desc_module
                .and_then(|dm| dm["text"].as_str())
                .map(|s| s.to_string());
            let original_post_id = dynamic_module
                .and_then(|dm| dm["dyn_forward"]["item"]["id_str"].as_str())
                .and_then(|s| s.parse::<u64>().ok());
            (DynamicType::Forward, original_post_id, None, None, content)
        }
        "DYNAMIC_TYPE_AV" => {
            let video_info = dynamic_module
                .and_then(|dm| dm.get("dyn_archive"))
                .map(|archive| VideoInfo {
                    title: archive["title"].as_str().unwrap_or("").to_string(),
                    description: archive["desc"].as_str().unwrap_or("").to_string(),
                    cover: archive["cover"].as_str().unwrap_or("").to_string(),
                    jump_url: archive["jump_url"].as_str().unwrap_or("").to_string(),
                    duration: archive["duration_text"].as_str().unwrap_or("").to_string(),
                    avid: archive["aid"].as_str().unwrap_or("").to_string(),
                    bvid: archive["bvid"].as_str().unwrap_or("").to_string(),
                    stats: archive["stat"].clone(),
                });
            let content = desc_module
                .and_then(|dm| dm["text"].as_str())
                .map(|s| s.to_string());
            (DynamicType::Video, None, video_info, None, content)
        }
        "DYNAMIC_TYPE_DRAW" => {
            let content = desc_module
                .and_then(|dm| dm["text"].as_str())
                .or_else(|| {
                    dynamic_module.and_then(|dm| dm["major"]["opus"]["summary"]["text"].as_str())
                })
                .map(|s| s.to_string());
            let image_urls = dynamic_module
                .and_then(|dm| dm["dyn_draw"]["items"].as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|i| i["src"].as_str().map(|s| s.to_string()))
                        .collect()
                });
            (DynamicType::Image, None, None, image_urls, content)
        }
        "DYNAMIC_TYPE_WORD" => {
            let content = desc_module
                .and_then(|dm| dm["text"].as_str())
                .map(|s| s.to_string());
            (DynamicType::Text, None, None, None, content)
        }
        _ => (DynamicType::Other, None, None, None, None),
    }
}

fn parse_old_version(
    item_type: &str,
    dynamic_module: Option<&Value>,
    desc_module: Option<&Value>,
    item: &Value,
) -> (
    DynamicType,
    Option<u64>,
    Option<VideoInfo>,
    Option<Vec<String>>,
    Option<String>,
) {
    match item_type {
        "DYNAMIC_TYPE_FORWARD" => {
            let content = desc_module
                .and_then(|dm| dm["text"].as_str())
                .map(|s| s.to_string());
            let original_post_id = item
                .get("orig")
                .and_then(|orig| orig["id_str"].as_str())
                .or_else(|| {
                    dynamic_module.and_then(|dm| dm["dyn_forward"]["item"]["id_str"].as_str())
                })
                .and_then(|s| s.parse::<u64>().ok());
            (DynamicType::Forward, original_post_id, None, None, content)
        }
        "DYNAMIC_TYPE_AV" => {
            let content = dynamic_module
                .and_then(|dm| dm["desc"]["text"].as_str())
                .map(|s| s.to_string());
            let video_info = dynamic_module
                .and_then(|dm| dm.get("major"))
                .and_then(|major| major.get("archive"))
                .map(|archive| VideoInfo {
                    title: archive["title"].as_str().unwrap_or("").to_string(),
                    description: archive["desc"].as_str().unwrap_or("").to_string(),
                    cover: archive["cover"].as_str().unwrap_or("").to_string(),
                    jump_url: archive["jump_url"].as_str().unwrap_or("").to_string(),
                    duration: archive["duration_text"].as_str().unwrap_or("").to_string(),
                    avid: archive["aid"].as_str().unwrap_or("").to_string(),
                    bvid: archive["bvid"].as_str().unwrap_or("").to_string(),
                    stats: archive["stat"].clone(),
                });
            (DynamicType::Video, None, video_info, None, content)
        }
        "DYNAMIC_TYPE_DRAW" => {
            let content = desc_module
                .and_then(|dm| dm["text"].as_str())
                .or_else(|| {
                    dynamic_module.and_then(|dm| dm["major"]["opus"]["summary"]["text"].as_str())
                })
                .map(|s| s.to_string());
            let image_urls = dynamic_module
                .and_then(|dm| dm.get("major"))
                .and_then(|major| {
                    if major["draw"]["items"].is_array() {
                        major["draw"]["items"].as_array()
                    } else if major["opus"]["pics"].is_array() {
                        major["opus"]["pics"].as_array()
                    } else {
                        None
                    }
                })
                .or_else(|| dynamic_module.and_then(|dm| dm["dyn_draw"]["items"].as_array()))
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|i| {
                            i["src"]
                                .as_str()
                                .or_else(|| i["url"].as_str())
                                .map(|s| s.to_string())
                        })
                        .collect()
                });
            if image_urls.is_some() {
                (DynamicType::Image, None, None, image_urls, content)
            } else {
                (DynamicType::Text, None, None, None, content)
            }
        }
        "DYNAMIC_TYPE_OPUS" => {
            let has_images = dynamic_module
                .and_then(|dm| dm["major"]["opus"]["pics"].as_array())
                .map(|pics| pics.len() > 0)
                .unwrap_or(false);
            let content = dynamic_module
                .and_then(|dm| dm["major"]["opus"]["summary"]["text"].as_str())
                .map(|s| s.to_string());
            if has_images {
                let image_urls = dynamic_module
                    .and_then(|dm| dm["major"]["opus"]["pics"].as_array())
                    .map(|pics| {
                        pics.iter()
                            .filter_map(|p| p["src"].as_str().map(|s| s.to_string()))
                            .collect()
                    });
                (DynamicType::Image, None, None, image_urls, content)
            } else {
                (DynamicType::Text, None, None, None, content)
            }
        }
        "DYNAMIC_TYPE_WORD" => {
            let content = desc_module
                .and_then(|dm| dm["text"].as_str())
                .map(|s| s.to_string());
            (DynamicType::Text, None, None, None, content)
        }
        _ => (DynamicType::Other, None, None, None, None),
    }
}
