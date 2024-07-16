use anyhow::{Context, Result};
use clap::Parser;
use regex::Regex;
use reqwest::blocking::get;
use scraper::{Html, Selector};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{create_dir_all, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

static URL_BASE: &str = "https://www.raiplaysound.it";

/// Simple command line tool
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// URL of the HTML page
    #[arg(short, long)]
    url: String,

    /// Path to the local folder
    #[arg(short, long, default_value = ".")]
    folder: PathBuf,

    /// Path to the cache folder
    #[arg(short, long, default_value_t = std::env::temp_dir().to_str().unwrap().to_string())]
    cache: String,
}

#[derive(Debug)]
struct AudioMetadata {
    url: String,
    title: String,
}

/// Fetches the HTML content from the URL or reads it from the cache if available.
fn fetch_or_read_page(url: &str, cache_dir: &Path) -> Result<String> {
    let (_, rawfilename) = url
        .rsplit_once('/')
        .with_context(|| format!("Failed to extract page name from: {}", url))?;
    let filename = format!("{}.html", rawfilename);
    let filepath = cache_dir.join(filename);

    if filepath.exists() {
        let mut file = File::open(&filepath)
            .with_context(|| format!("Failed to open file: {}", filepath.display()))?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .with_context(|| format!("Failed to read file: {}", filepath.display()))?;
        Ok(contents)
    } else {
        let response = get(url)
            .with_context(|| format!("Failed to fetch URL: {}", url))?
            .text()
            .with_context(|| format!("Failed to get text from URL: {}", url))?;
        let mut file = File::create(&filepath)
            .with_context(|| format!("Failed to create file: {}", filepath.display()))?;
        file.write_all(response.as_bytes())
            .with_context(|| format!("Failed to write to file: {}", filepath.display()))?;
        Ok(response)
    }
}

/// Extracts audio options from the HTML content.
fn extract_options(html: &str) -> Vec<String> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("rps-play-with-labels").expect("Invalid selector");

    document
        .select(&selector)
        .filter_map(|element| {
            if let Some(options) = element.value().attr("options") {
                let options_map: HashMap<String, Value> = serde_json::from_str(options)
                    .expect("Unable to parse JSON in options attribute");
                if let Some(url) = options_map.get("url") {
                    return Some(url.as_str().unwrap().to_string());
                }
            }
            None
        })
        .collect()
}

/// Fetches audio metadata from the given URL or reads it from the cache if available.
fn fetch_audio_metadata(url: &str, cache_dir: &Path) -> Result<AudioMetadata> {
    let full_url = format!("{}{}", URL_BASE, url);
    let (_, filename) = full_url
        .rsplit_once('/')
        .with_context(|| format!("Failed to extract file name from: {}", full_url))?;
    let filepath = cache_dir.join(filename);

    let json_content = if filepath.exists() {
        let mut file = File::open(&filepath)
            .with_context(|| format!("Failed to open file: {}", filepath.display()))?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .with_context(|| format!("Failed to read file: {}", filepath.display()))?;
        contents
    } else {
        let response = get(&full_url)
            .with_context(|| format!("Failed to fetch URL: {}", full_url))?
            .text()
            .with_context(|| format!("Failed to get text from URL: {}", full_url))?;
        let mut file = File::create(&filepath)
            .with_context(|| format!("Failed to create file: {}", filepath.display()))?;
        file.write_all(response.as_bytes())
            .with_context(|| format!("Failed to write to file: {}", filepath.display()))?;
        response
    };

    let json_value: Value = serde_json::from_str(&json_content)
        .with_context(|| format!("Failed to parse JSON: {}", full_url))?;

    let audio_url = json_value["audio"]["url"]
        .as_str()
        .context("Missing field `url`")?
        .to_string();
    let audio_title = json_value["audio"]["title"]
        .as_str()
        .context("Missing field `title`")?
        .to_string();

    Ok(AudioMetadata {
        url: audio_url,
        title: audio_title,
    })
}

/// Downloads audio from the given metadata and saves it to the specified folder.
fn download_audio(metadata: &AudioMetadata, folder: &Path, idx: usize) -> Result<()> {
    let re = Regex::new(r"[^\w\s-]")?;
    let sanitized_title = re.replace_all(&metadata.title, "_").to_lowercase();
    let output_path = folder.join(format!("{:03} - {}.mp3", idx, sanitized_title));

    if let Some(folder_path) = output_path.parent() {
        create_dir_all(folder_path).with_context(|| {
            format!("Failed to create parent folder: {}", folder_path.display())
        })?;
    };

    if output_path.exists() {
        println!(
            "File {} already exists. Skipping download.",
            output_path.display()
        );
        return Ok(());
    }

    let response = get(&metadata.url)
        .with_context(|| format!("Failed to fetch audio URL: {}", metadata.url))?;
    let mut file = File::create(&output_path).with_context(|| {
        format!(
            "Failed to create file: {}. Error: {:?}",
            output_path.display(),
            std::io::Error::last_os_error()
        )
    })?;
    file.write_all(&response.bytes()?).with_context(|| {
        format!(
            "Failed to write to file: {}. Error: {:?}",
            output_path.display(),
            std::io::Error::last_os_error()
        )
    })?;
    println!("Downloaded {} to {}", metadata.title, output_path.display());
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    let cache_dir = PathBuf::from(&args.cache);

    let html_content = fetch_or_read_page(&args.url, &cache_dir)?;

    for (idx, url) in extract_options(&html_content).into_iter().enumerate() {
        match fetch_audio_metadata(&url, &cache_dir) {
            Ok(metadata) => {
                if let Err(e) = download_audio(&metadata, &args.folder, idx) {
                    eprintln!("Failed to download audio: {}", e);
                }
            }
            Err(e) => eprintln!("Failed to fetch audio metadata: {}", e),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_or_read_page() -> Result<()> {
        let url = "https://example.com";
        let cache_dir = std::env::temp_dir().join("test_cache");
        create_dir_all(&cache_dir)?;

        let result = fetch_or_read_page(url, &cache_dir);
        assert!(result.is_ok());

        // Check that the file was cached
        let filename = format!("{}.html", base64::encode(url));
        let filepath = cache_dir.join(filename);
        assert!(filepath.exists());

        Ok(())
    }

    #[test]
    fn test_extract_options() {
        let html = r#"<rps-play-with-labels options='{"url": "audio/2015/06/I-tre-moschettieri---Lettura-I-2c45793e-a289-42a8-97ae-656a2a94a71f.json"}'></rps-play-with-labels>"#;
        let options = extract_options(html);
        assert_eq!(options.len(), 1);
        assert_eq!(options[0], "audio/2015/06/I-tre-moschettieri---Lettura-I-2c45793e-a289-42a8-97ae-656a2a94a71f.json");
    }

    #[test]
    fn test_fetch_audio_metadata() -> Result<()> {
        let url = "/audio/2015/06/I-tre-moschettieri---Lettura-I-2c45793e-a289-42a8-97ae-656a2a94a71f.json";
        let cache_dir = std::env::temp_dir().join("test_cache");
        create_dir_all(&cache_dir)?;

        let metadata = fetch_audio_metadata(url, &cache_dir)?;
        assert!(!metadata.url.is_empty());
        assert!(!metadata.title.is_empty());

        Ok(())
    }

    #[test]
    fn test_download_audio() -> Result<()> {
        let metadata = AudioMetadata {
            url: "https://mediapolisvod.rai.it/relinker/relinkerServlet.htm?cont=jmC2BrdAhSIeeqqEEqual".to_string(),
            title: "Test Audio".to_string(),
        };
        let folder = std::env::temp_dir().join("test_audio");
        create_dir_all(&folder)?;

        let result = download_audio(&metadata, &folder, 1);
        assert!(result.is_ok());

        let re = Regex::new(r"[^\w\s-]")?;
        let sanitized_title = re.replace_all(&metadata.title, "_").to_lowercase();
        let output_path = folder.join(format!("{:03} - {}.mp3", 1, sanitized_title));

        assert!(output_path.exists());

        Ok(())
    }
}
