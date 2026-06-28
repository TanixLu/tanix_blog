use std::{fs::File, path::PathBuf, str::FromStr, sync::LazyLock};

use comrak::{
    Arena,
    nodes::{AstNode, NodeValue},
    parse_document,
};
use constcat::concat;
use tera::Tera;
use zip::ZipArchive;

use crate::utils::human_path_size;

const BLOG_PATH: &str = "blog/";
const FACICON_ZIP_PATH: &str = concat!(BLOG_PATH, "favicon.zip");
const MD_POSTS_PATH: &str = concat!(BLOG_PATH, "posts/");
const TEMPLATES_GLOB: &str = concat!(BLOG_PATH, "templates/**/*.html");
const NOT_FOUND_TEMPLATE: &str = "404.html";
const POST_TEMPLATE: &str = "post.html";
const INDEX_TEMPLATE: &str = "index.html";
const PUBLIC_PATH: &str = "public/";
const POSTS_PATH: &str = concat!(PUBLIC_PATH, "posts/");
const NOT_FOUND_PATH: &str = concat!(PUBLIC_PATH, NOT_FOUND_TEMPLATE);
const INDEX_PATH: &str = concat!(PUBLIC_PATH, INDEX_TEMPLATE);
const FRONT_MATTER_DELIMITER: &str = "---";
const DESCRIPTION_MAX_CHARS: usize = 160;

static BLOG_ROOT: LazyLock<String> = LazyLock::new(|| std::env::var("BLOG_ROOT").unwrap());
static BLOG_NAME: LazyLock<String> = LazyLock::new(|| std::env::var("BLOG_NAME").unwrap());

static COMRAK_OPTIONS: LazyLock<comrak::Options> = LazyLock::new(|| {
    let mut options = comrak::Options::default();
    options.extension.front_matter_delimiter = Some(FRONT_MATTER_DELIMITER.to_string());
    options
});

fn parse_title_and_date(markdown: &str) -> anyhow::Result<(String, NaiveDate)> {
    let front_matter = markdown
        .split(FRONT_MATTER_DELIMITER)
        .filter(|s| !s.trim().is_empty())
        .next()
        .unwrap();

    let mut title = None;
    let mut date = None;

    for line in front_matter.lines() {
        if let Some((key, value)) = line.trim().split_once(":") {
            match key.trim() {
                "title" => title = Some(value.trim().to_string()),
                "date" => date = Some(NaiveDate::from_str(value.trim())?),
                _ => (),
            }
        }
    }

    Ok((title.unwrap(), date.unwrap()))
}

fn md_description(md: &str) -> anyhow::Result<String> {
    let arena = Arena::new();
    let root = parse_document(&arena, md, &COMRAK_OPTIONS);

    // descendants() 按文档顺序遍历；第一个 Paragraph 即第一段正文（标题是 Heading，会跳过）
    let para = root
        .descendants()
        .find(|n| matches!(n.data.borrow().value, NodeValue::Paragraph))
        .unwrap();

    // 递归收集纯文本
    let mut text = String::new();
    collect_text(para, &mut text);

    let text: String = text
        .trim()
        .lines()
        .next()
        .unwrap()
        .trim()
        .chars()
        .take(DESCRIPTION_MAX_CHARS)
        .collect();

    Ok(text)
}

/// 递归收集一个节点下的所有内联文本
fn collect_text<'a>(node: &'a AstNode<'a>, out: &mut String) {
    match &node.data.borrow().value {
        NodeValue::Text(t) => out.push_str(t),
        NodeValue::Code(c) => out.push_str(&c.literal),
        NodeValue::SoftBreak | NodeValue::LineBreak => {}
        _ => {
            for child in node.children() {
                collect_text(child, out);
            }
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct PageMeta {
    title: String,
    description: String,
    og_type: String,
    canonical_url: String,
}

impl PageMeta {
    fn index_meta() -> Self {
        PageMeta {
            title: BLOG_NAME.clone(),
            description: BLOG_NAME.clone(),
            og_type: "website".to_string(),
            canonical_url: BLOG_ROOT.clone(),
        }
    }
}

struct NaiveDate {
    year: u32,
    month: u32,
    day: u32,
}

impl FromStr for NaiveDate {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(3, '-');
        let year = parts.next().unwrap().parse().unwrap();
        let month = parts.next().unwrap().parse().unwrap();
        let day = parts.next().unwrap().parse().unwrap();
        Ok(NaiveDate { year, month, day })
    }
}

impl serde::Serialize for NaiveDate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{}-{}-{}", self.year, self.month, self.day))
    }
}

impl<'de> serde::Deserialize<'de> for NaiveDate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        NaiveDate::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Post {
    title: String,
    date: NaiveDate,
    description: String,
    id: String,
    canonical_url: String,
    html_content: String,
}

impl Post {
    fn parse_md(path: PathBuf) -> anyhow::Result<Self> {
        let markdown = std::fs::read_to_string(&path)?;
        let (title, date) = parse_title_and_date(&markdown)?;
        let description = md_description(&markdown)?;
        let id = path.file_prefix().unwrap().to_str().unwrap().to_string();
        let canonical_url = format!("{}posts/{}", *BLOG_ROOT, id);
        let html_content = comrak::markdown_to_html(&markdown, &COMRAK_OPTIONS);
        Ok(Post {
            title,
            date,
            description,
            id,
            canonical_url,
            html_content,
        })
    }

    fn html_path(&self) -> String {
        format!("{}/posts/{}.html", PUBLIC_PATH, self.id)
    }

    fn page_meta(&self) -> PageMeta {
        PageMeta {
            title: self.title.clone(),
            description: self.description.clone(),
            og_type: "article".into(),
            canonical_url: self.canonical_url.clone(),
        }
    }
}

pub fn build_html() -> anyhow::Result<()> {
    // 清空原有public文件
    let _ = std::fs::remove_dir_all(PUBLIC_PATH);
    std::fs::create_dir_all(PUBLIC_PATH)?;
    std::fs::create_dir_all(POSTS_PATH)?;

    // 解压favicon.zip
    let favicon_zip = File::open(FACICON_ZIP_PATH)?;
    let mut archive = ZipArchive::new(favicon_zip)?;
    archive.extract(PUBLIC_PATH)?;

    // 读取tera模板
    let mut tera = Tera::new();
    tera.load_from_glob(TEMPLATES_GLOB)?;

    // 生成404页面
    std::fs::write(
        NOT_FOUND_PATH,
        tera.render(NOT_FOUND_TEMPLATE, &tera::Context::new())?,
    )?;

    // 读取posts
    let mut posts: Vec<Post> = Vec::new();
    for entry in std::fs::read_dir(MD_POSTS_PATH)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
            posts.push(Post::parse_md(path)?);
        }
    }

    // 生成posts
    for post in posts.iter() {
        let mut tera_context = tera::Context::new();
        tera_context.insert("page_meta", &post.page_meta());
        tera_context.insert("post", &post);
        std::fs::write(
            &post.html_path(),
            tera.render(POST_TEMPLATE, &tera_context)?,
        )?;
        println!(
            "Generated HTML for post: {} {}, {}.",
            post.id,
            post.title,
            human_path_size(&post.html_path())?
        );
    }

    // 生成index
    let mut tera_context = tera::Context::new();
    tera_context.insert("page_meta", &PageMeta::index_meta());
    tera_context.insert("posts", &posts);
    std::fs::write(INDEX_PATH, tera.render(INDEX_TEMPLATE, &tera_context)?)?;

    println!(
        "HTML built successfully, {}.",
        human_path_size(PUBLIC_PATH)?
    );
    Ok(())
}
