use chrono::NaiveDate;
use glob::glob;
use pulldown_cmark::{html, Event, LinkType, Options, Parser, Tag};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fmt, fs,
    io::Write,
    path::PathBuf,
};

use crate::{cache::UrlCache, hash::get_hash};

#[derive(Debug)]
pub enum ContentsError {
    FileRead,
    FileWrite,
    MdParse,
    FetchOgpError,
    OtherBuild,
}

impl fmt::Display for ContentsError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ContentsError::FileRead => write!(f, "ContentsError::FileRead"),
            ContentsError::FileWrite => write!(f, "ContentsError::FileWrite"),
            ContentsError::MdParse => write!(f, "ContentsError::MdParse"),
            ContentsError::FetchOgpError => write!(f, "ContentsError::FetchOgpError"),
            ContentsError::OtherBuild => write!(f, "ContentsError::OtherBuild"),
        }
    }
}

pub struct ContentMeta {
    pub title: String,
    pub slug: String,
    pub date: chrono::NaiveDate,
    pub tags: Vec<String>,
    pub images: Vec<String>,
}

pub struct Content {
    pub meta: ContentMeta,
    pub content: String,
}

pub fn list_mds_from_dir(
    src_dir: &str,
) -> Result<HashMap<String, (Vec<u8>, String)>, ContentsError> {
    glob(format!("{}/**/*.md", src_dir).as_str())
        .map_err(|_| ContentsError::FileRead)?
        .filter_map(Result::ok)
        .map(|v| {
            let file_str = fs::read_to_string(&v).map_err(|_| ContentsError::FileRead)?;
            let hash = get_hash(&file_str);
            let path = v.to_str().ok_or(ContentsError::OtherBuild)?.to_string();
            Ok((path, (hash, file_str)))
        })
        .collect::<Result<HashMap<_, _>, ContentsError>>()
}

pub async fn get_md_data(
    file_str: &str,
    url_caches: &mut HashMap<String, UrlCache>,
    http_client: reqwest::Client,
    ogp_api_endpoint: &str,
) -> Result<Content, ContentsError> {
    #[derive(Deserialize)]
    struct ContentFrontmatter {
        pub title: String,
        pub slug: String,
        pub date: String,
        pub tags: Vec<String>,
    }

    #[derive(Deserialize)]
    struct OgpApiUrl {
        pub title: String,
        pub description: Option<String>,
        pub image: Option<String>,
        pub favicon: Option<String>,
    }

    #[derive(Serialize)]
    struct OgpApiRequest {
        pub url: String,
    }

    let mut local_images: HashSet<String> = HashSet::new();

    let mut link_urls: HashSet<String> = HashSet::new();

    let url_list_parser = Parser::new_ext(file_str, Options::all()).map(|event| match event {
        Event::Start(Tag::Link(link_type, url, child)) => {
            if link_type == LinkType::Autolink {
                link_urls.insert(url.clone().into_string());
            }
            Event::Start(Tag::Link(link_type, url, child))
        }
        _ => event,
    });

    let mut html_buf = String::new();
    html::push_html(&mut html_buf, url_list_parser);

    for link in link_urls.into_iter() {
        let ogp = http_client
            .post(ogp_api_endpoint)
            .json(&OgpApiRequest { url: link.clone() })
            .send()
            .await
            .map_err(|_| ContentsError::FetchOgpError)?
            .json::<OgpApiUrl>()
            .await
            .map_err(|_| ContentsError::FetchOgpError)?;
        url_caches.insert(
            link.clone(),
            UrlCache {
                url: link,
                title: ogp.title,
                description: ogp.description,
                image: ogp.image,
                favicon: ogp.favicon,
            },
        );
    }

    let mut hoge = 0;
    let mut front = String::new();
    let mut content = String::new();

    for str in file_str.split('\n') {
        if str.ends_with("--") {
            hoge += 1
        } else if hoge <= 1 {
            front += format!("{}\n", str).as_str();
        } else {
            content += format!("{}\n", str).as_str();
        }
    }

    let parser = Parser::new_ext(&content, Options::all()).map(|event| match event {
        Event::Start(Tag::Image(link_type, url, title)) => {
            if url.starts_with("./") {
                local_images.insert(url.clone().into_string());
            }
            Event::Start(Tag::Image(link_type, url, title))
        }
        Event::Start(Tag::Link(link_type, url, child)) => {
            if link_type == LinkType::Autolink {
                let ogp = url_caches.get(&url.to_string());
                let mut html = include_str!("template/component/a.html")
                    .replace("{{url}}", &url)
                    .replace("{{child}}", &child);

                if let Some(v) = ogp {
                    let favicon = match &v.favicon {
                        Some(favicon) => {
                            format!("<img class=\"link_card_favicon\" src=\"{}\" />", *favicon)
                        }
                        None => "".to_owned(),
                    };
                    html = include_str!("template/component/link_card.html")
                        .replace("{{title}}", &v.title)
                        .replace(
                            "{{description}}",
                            match &v.description {
                                Some(v) => v,
                                None => "",
                            },
                        )
                        .replace("{{url}}", &v.url)
                        .replace("{{favicon_img}}", &favicon);
                    if let Some(img) = &v.image {
                        html = html.replace(
                            "{{image}}",
                            include_str!("template/component/link_card_image.html")
                                .replace("{{image_url}}", img)
                                .as_str(),
                        )
                    }
                }
                Event::Html(html.into())
            } else {
                Event::Html(
                    include_str!("template/component/a.html")
                        .replace("{{url}}", &url)
                        .replace("{{child}}", &child)
                        .into(),
                )
            }
        }
        Event::Text(text) => match url_caches.get(&text.to_string()) {
            Some(_) => Event::Text("".into()),
            _ => Event::Text(text),
        },
        _ => event,
    });

    let mut html_buf = String::new();
    html::push_html(&mut html_buf, parser);

    let meta = serde_yaml::from_str::<ContentFrontmatter>(&front)
        .map(|v| {
            NaiveDate::parse_from_str(&v.date, "%Y/%m/%d")
                .map(|date| ContentMeta {
                    title: v.title,
                    slug: v.slug,
                    date,
                    images: Vec::from_iter(local_images),
                    tags: v.tags,
                })
                .map_err(|_| ContentsError::MdParse)
        })
        .map_err(|_| ContentsError::MdParse)??;

    Ok(Content {
        meta,
        content: html_buf,
    })
}

impl Content {
    pub fn build_md(&self, src_dir: &str, dist_dir: &str) -> Result<Vec<u8>, ContentsError> {
        let html = include_str!("template/post.html")
            .replace("{{title}}", &self.meta.title)
            .replace("{{date}}", &self.meta.date.format("%Y/%m/%d").to_string())
            .replace("{{content}}", &self.content)
            .split('\n')
            .collect::<Vec<_>>()
            .iter()
            .map(|v| v.trim())
            .collect::<Vec<_>>()
            .join("");
        let hash = get_hash(&html);
        let dir = format!("{}/{}", dist_dir.trim_end_matches('/'), self.meta.slug)
            .trim_end_matches('/')
            .to_owned();
        fs::create_dir_all(&dir).map_err(|_| ContentsError::FileWrite)?;
        let _ = self
            .meta
            .images
            .iter()
            .map(|v| {
                let _ = fs::create_dir_all(format!(
                    "{}/{}",
                    dir,
                    PathBuf::from(v).parent().unwrap().to_str().unwrap(),
                ));
                // Result を潰す
                let _ = std::fs::copy(
                    format!(
                        "{}/{}/{}",
                        src_dir.trim_end_matches('/'),
                        self.meta.slug,
                        v.replace("./", "")
                    ),
                    format!("{}/{}", dir, v.replace("./", "")),
                );
            })
            .collect::<Vec<_>>();
        fs::File::create(format!("{}/index.html", &dir))
            .map(|mut v| v.write_all(html.as_bytes()))
            .map(|_| hash)
            .map_err(|_| ContentsError::FileWrite)
    }
}
