use crypto::digest::Digest;
use crypto::md5::Md5;
use futures::future::join_all;
use lz_str::decompress_from_base64;
use regex::Regex;
use reqwest::header::HeaderMap;
use soup::prelude::*;
use std::char::{decode_utf16, REPLACEMENT_CHARACTER};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
// use std::env;

static VERSION: &str = "manhuacat2021";

async fn get_picture_download_url(
    resq: &reqwest::Client,
    url: String,
    picture_number: String,
    save_path: String,
) -> Result<(), Box<dyn std::error::Error>> {
    // 组装header
    let mut headers = HeaderMap::new();
    headers.insert("referer", "https://www.manhuacat.com/".parse().unwrap());
    let start = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .to_string()
        .chars()
        .take(10)
        .collect::<String>();
    let mut md5 = Md5::new();
    let final_str = format!("{}{}{}", &url, &VERSION, &start);
    md5.input_str(&final_str);
    let result = md5.result_str();
    let download_url = format!(
        "https://mao.mhtupian.com/uploads/{0}?_MD={1}&_TM={2}",
        &url, result, start
    );
    let dirs = fs::read_dir(format!("{}", &save_path));
    if dirs.is_err() {
        if fs::create_dir_all(format!("{}", &save_path)).is_ok() {
            println!("{}目录创建成功!", &save_path);
        }
    }
    if fs::File::open(format!("{}/{}", &save_path, &picture_number)).is_err() {
        let req = resq
            .get(download_url)
            .headers(headers)
            .send()
            .await?
            .bytes()
            .await?;
        if fs::write(format!("{}/{}", &save_path, &picture_number), &req).is_ok() {
            println!("{}下载成功!", format!("{}/{}", &save_path, &picture_number));
        }
    } else {
        println!("{}已存在!", format!("{}/{}", &save_path, &picture_number));
    }
    Ok(())
}

async fn get_picture_download_list(
    resq: &reqwest::Client,
    url: String,
    save_path: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let resp = resq.get(url).send().await?.text().await?;
    let match_ = Regex::new("img_data = \"(.+?)\"").unwrap();
    for chapter in match_.captures_iter(&resp) {
        let decode = decompress_from_base64(&chapter[1]).unwrap();
        let all_picture_url = decode_utf16(decode.iter().cloned())
            .map(|r| r.unwrap_or(REPLACEMENT_CHARACTER))
            .collect::<String>();
        let all_picture_url = all_picture_url.split(",");
        let mut v = Vec::new();
        for url in all_picture_url {
            let picture_number = url.split("/").last().unwrap();
            v.push(get_picture_download_url(
                resq,
                url.parse().unwrap(),
                picture_number.to_string(),
                save_path.to_string(),
            ));
        }
        join_all(v).await;
    }
    Ok(())
}

async fn get_chapter(
    resq: &reqwest::Client,
    url: String,
    save_path: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let resp = resq.get(url).send().await?.text().await?;
    let soup = Soup::new(&resp);
    let info = soup.tag("a").attr("class", "fixed-a-es").find_all();
    let mut v = Vec::new();
    for item in info {
        let comic_url = item.get("href");
        let save_path = format!("{}/{}", save_path, item.get("title").unwrap());
        v.push(get_picture_download_list(
            resq,
            comic_url.unwrap(),
            save_path,
        ));
    }
    join_all(v).await;
    Ok(())
}

async fn get_one_commic(
    client: &reqwest::Client,
    url: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let resp = client.get(url).send().await?.text().await?;
    let soup = Soup::new(&resp);

    let title = soup
        .tag("div")
        .attr("class", "media")
        .attr("class", "comic-book-unit")
        .find_all();
    let mut v = Vec::new();
    for item in title {
        let a_link = item
            .tag("a")
            .attr("class", "d-block")
            .find()
            .expect("not found");
        let comic_name = a_link
            .children()
            .next()
            .unwrap()
            .get("alt")
            .unwrap()
            .replace("封皮", "");
        v.push(get_chapter(
            client,
            a_link.get("href").unwrap(),
            format!("./comic/{}", comic_name),
        ));
    }
    join_all(v).await;
    Ok(())
}

async fn get_all_pages(client: reqwest::Client) -> Result<(), Box<dyn std::error::Error>> {
    let resp = &client
        .get("https://www.manhuacat.com/list-page-1.html")
        .send()
        .await?
        .text()
        .await?;
    let soup = Soup::new(&resp);
    let last_page_index = soup
        .tag("a")
        .attr("class", "btn")
        .attr("class", "btn-light")
        .attr("class", "mr-1")
        .attr("class", "mb-1")
        .find_all()
        .into_iter()
        .last()
        .unwrap()
        .text();
    let mut v = Vec::new();
    for index in 1..=last_page_index.parse().unwrap() {
        // get_one_commic(
        //     &client,
        //     format!("https://www.manhuacat.com/list-page-{}.html", index),
        // ).await?;
        v.push(get_one_commic(
            &client,
            format!("https://www.manhuacat.com/list-page-{}.html", index),
        ));
    }
    join_all(v).await;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    get_all_pages(client).await?;
    Ok(())
}

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let args: Vec<String> = env::args().collect();
//     let cli = reqwest::Client::new();
//     get_chapter(&cli, (*args[2]).parse().unwrap(), format!("./comic/{}", args[1])).await?;
//     Ok(())
// }
