use futures::future::join_all;
use lz_str::decompress_from_base64;
use regex::Regex;
use reqwest::header::HeaderMap;
use select::document::Document;
use select::predicate::{Class, Name, Predicate};
use std::char::{decode_utf16, REPLACEMENT_CHARACTER};
use std::fs;

async fn get_picture_download_url(
    client: reqwest::Client,
    url: String,
    picture_number: String,
    save_path: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 组装header
    let mut headers = HeaderMap::new();
    headers.insert("referer", "https://www.maofly.com/".parse().unwrap());
    let download_url = format!("https://mao.mhtupian.com/uploads/{}", &url);
    let dirs = fs::read_dir(format!("{}", &save_path));
    if dirs.is_err() {
        if fs::create_dir_all(format!("{}", &save_path)).is_ok() {
            println!("{}目录创建成功!", &save_path);
        }
    }
    println!("下载地址: {}", download_url);
    if fs::File::open(format!("{}/{}", &save_path, &picture_number)).is_err() {
        let req = client
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
// async fn get_picture_download_url(
//     client: reqwest::Client,
//     url: String,
//     picture_number: String,
//     save_path: String,
// ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//     // 组装header
//     // let mut headers = HeaderMap::new();
//     // headers.insert("referer", "https://www.manhuacat.com/".parse().unwrap());
//     let start = SystemTime::now()
//         .duration_since(UNIX_EPOCH)
//         .unwrap()
//         .as_millis()
//         .to_string()
//         .chars()
//         .take(10)
//         .collect::<String>();
//     let mut md5 = Md5::new();
//     let final_str = format!("{}{}{}", &url, &VERSION, &start);
//     md5.input_str(&final_str);
//     let result = md5.result_str();
//     let download_url = format!(
//         "https://mao.mhtupian.com/uploads/{0}?_MD={1}&_TM={2}",
//         &url, result, start
//     );
//     let dirs = fs::read_dir(format!("{}", &save_path));
//     if dirs.is_err() {
//         if fs::create_dir_all(format!("{}", &save_path)).is_ok() {
//             println!("{}目录创建成功!", &save_path);
//         }
//     }
//     if fs::File::open(format!("{}/{}", &save_path, &picture_number)).is_err() {
//         let req = client.get(download_url).send().await?.bytes().await?;
//         if fs::write(format!("{}/{}", &save_path, &picture_number), &req).is_ok() {
//             println!("{}下载成功!", format!("{}/{}", &save_path, &picture_number));
//         }
//     } else {
//         println!("{}已存在!", format!("{}/{}", &save_path, &picture_number));
//     }
//     Ok(())
// }

async fn get_picture_download_list(
    client: reqwest::Client,
    url: String,
    save_path: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let resp = client.get(url).send().await?.text().await?;
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
            v.push(tokio::spawn(get_picture_download_url(
                client.clone(),
                url.parse().unwrap(),
                picture_number.to_string(),
                save_path.to_string(),
            )));
        }
        join_all(v).await;
    }
    Ok(())
}

async fn get_chapter(
    client: reqwest::Client,
    url: String,
    save_path: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let resp = client.get(url).send().await?.text().await?;
    let document = Document::from(&resp[..]);
    let info = document.find(Name("a").and(Class("fixed-a-es")));
    let mut v = Vec::new();
    for item in info {
        let comic_url = item.attr("href").unwrap();
        let save_path = format!("{}/{}", save_path, item.attr("title").unwrap());
        v.push(tokio::spawn(get_picture_download_list(
            client.clone(),
            comic_url.to_string(),
            save_path,
        )));
    }
    join_all(v).await;
    // for item in v{
    //     item.await;
    // }
    Ok(())
}

async fn get_one_comic(
    client: reqwest::Client,
    url: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let resp = client.get(url).send().await?.text().await?;
    let document = Document::from(&resp[..]);
    let title = document.find(
        Name("div")
            .and(Class("media"))
            .and(Class("comic-book-unit")),
    );
    let mut v = Vec::new();
    for item in title {
        let a_link = item.find(Name("a").and(Class("d-block"))).last().unwrap();
        let comic_name = a_link
            .children()
            .next()
            .unwrap()
            .attr("alt")
            .unwrap()
            .replace("封皮", "");
        v.push(get_chapter(
            client.clone(),
            a_link.attr("href").unwrap().to_string(),
            format!("./comic/{}", comic_name),
        ));
    }
    join_all(v).await;
    Ok(())
}

async fn get_all_pages(
    client: reqwest::Client,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let resp = client
        .get("https://www.maofly.com/list-page-1.html")
        .send()
        .await?
        .text()
        .await?;
    let document = Document::from(&resp[..]);
    let last_page_index = document
        .find(
            Name("a")
                .and(Class("btn"))
                .and(Class("btn-light"))
                .and(Class("mr-1"))
                .and(Class("mb-1")),
        )
        .last()
        .unwrap()
        .text();
    let mut v = Vec::new();
    let last_index: i32 = last_page_index.parse().unwrap();
    for index in 1..=last_index {
        v.push(get_one_comic(
            client.clone(),
            format!("https://www.maofly.com/list-page-{}.html", index),
        ));
    }
    join_all(v).await;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    // let mut headers = HeaderMap::new();
    // headers.insert("referer", "https://www.manhuacat.com/".parse().unwrap());
    // headers.insert(
    //     "user-agent",
    //     "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:90.0) Gecko/20100101 Firefox/90.0"
    //         .parse()
    //         .unwrap(),
    // );
    // let resp = client
    //     .get("https://www.manhuacat.com/list-page-1.html")
    //     .headers(headers)
    //     .send()
    //     .await?
    //     .text()
    //     .await?;
    // let return_url = &(Regex::new("window.location.href =\"(?P<returnUrl>.+?)\";")
    //     .unwrap()
    //     .captures(&resp)
    //     .unwrap())["returnUrl"];
    // client
    //     .get(format!("https://www.manhuacat.com{}", return_url))
    //     .send()
    //     .await?
    //     .text()
    //     .await?;
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
