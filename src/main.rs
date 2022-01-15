mod cache;
mod contents;
mod hash;

use std::{collections::HashMap, env};

use cache::{Cache, UrlCache};

use crate::{
    cache::write_cache,
    contents::{get_md_data, list_mds_from_dir},
};

#[tokio::main]
async fn main() {
    let mut args = env::args().skip(1);
    let src_dir = args.next().expect("入力ディレクトリが指定されてないよ");
    let dist_dir = args.next().expect("出力ディレクトリが指定されてないよ");
    let ogp_api_endpoint = args
        .next()
        .expect("OGP API エンドポイントが指定されてないよ");
    let caches = match cache::get_cache("cache.json") {
        Ok(v) => v,
        Err(_) => Cache {
            urls: HashMap::new(),
            contents: HashMap::new(),
        },
    };
    let mut url_caches: HashMap<String, UrlCache> = HashMap::new();
    let http_client = reqwest::Client::new();

    let mut hoge = list_mds_from_dir(&src_dir)
        .unwrap()
        .into_iter()
        .filter(|(path, (hash, _))| caches.contents.get(path) != Some(hash));
    let mut contents_cache: HashMap<String, Vec<u8>> = HashMap::new();
    for (path, (_, file_str)) in &mut hoge {
        if let Ok(v) = get_md_data(
            &file_str,
            &mut url_caches,
            http_client.clone(),
            &ogp_api_endpoint,
        )
        .await
        {
            let hash = v.build_md(&src_dir, &dist_dir).expect("ビルド失敗したよ");
            contents_cache.insert(path, hash);
        }
    }
    write_cache("cache.json", url_caches, contents_cache).expect("キャッシュ書き込み失敗したよ");
}
