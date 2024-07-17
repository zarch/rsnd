use anyhow::{Context, Result};
use clap::Parser;
use regex::Regex;
use reqwest::header::HeaderMap;
use reqwest::Client;
use scraper::{Html, Selector};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{create_dir_all, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use tokio::fs::File as TokioFile;
use tokio::io::AsyncWriteExt;

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
async fn fetch_or_read_page(client: &Client, url: &str, cache_dir: &Path) -> Result<String> {
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
        let response = client
            .get(url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch URL: {}", url))?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to fetch URL: {}. Status: {}",
                url,
                response.status()
            ));
        }

        let rsp_txt = response
            .text()
            .await
            .with_context(|| format!("Failed to get text from URL: {}", url))?;
        let mut file = TokioFile::create(&filepath)
            .await
            .with_context(|| format!("Failed to create file: {}", filepath.display()))?;
        file.write_all(rsp_txt.as_bytes())
            .await
            .with_context(|| format!("Failed to write to file: {}", filepath.display()))?;
        Ok(rsp_txt)
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
async fn fetch_audio_metadata(
    client: &Client,
    url: &str,
    cache_dir: &Path,
) -> Result<AudioMetadata> {
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
        let response = client
            .get(&full_url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch URL: {}", full_url))?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to fetch URL: {}. Status: {}",
                full_url,
                response.status()
            ));
        }

        let rsp_txt = response
            .text()
            .await
            .with_context(|| format!("Failed to get text from URL: {}", full_url))?;
        let mut file = TokioFile::create(&filepath)
            .await
            .with_context(|| format!("Failed to create file: {}", filepath.display()))?;
        file.write_all(rsp_txt.as_bytes())
            .await
            .with_context(|| format!("Failed to write to file: {}", filepath.display()))?;
        rsp_txt
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
async fn download_audio(
    client: &Client,
    metadata: &AudioMetadata,
    folder: &Path,
    idx: usize,
) -> Result<()> {
    let re = Regex::new(r"[^\w\s-]")?;
    let sanitized_title = re.replace_all(&metadata.title, "_").to_lowercase();
    let output_path = folder.join(format!("{:03} - {}.mp3", idx, sanitized_title));

    if output_path.exists() {
        println!(
            "File {} already exists. Skipping download.",
            output_path.display()
        );
        return Ok(());
    }

    let response = client
        .get(&metadata.url)
        .send()
        .await
        .with_context(|| format!("Failed to fetch audio URL: {}", metadata.url))?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to fetch audio URL: {}. Status: {}",
            metadata.url,
            response.status()
        ));
    }

    let mut file = TokioFile::create(&output_path).await.with_context(|| {
        format!(
            "Failed to create file: {}. Error: {:?}",
            output_path.display(),
            std::io::Error::last_os_error()
        )
    })?;
    file.write_all(&response.bytes().await?)
        .await
        .with_context(|| {
            format!(
                "Failed to write to file: {}. Error: {:?}",
                output_path.display(),
                std::io::Error::last_os_error()
            )
        })?;
    println!("Downloaded {} to {}", metadata.title, output_path.display());
    Ok(())
}

fn get_client() -> Result<Client> {
    let mut headers = HeaderMap::new();
    headers.insert("accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8".parse().unwrap());
    headers.insert("accept-language", "en-US,en;q=0.7".parse().unwrap());
    headers.insert("priority", "u=0, i".parse().unwrap());
    headers.insert(
        "sec-ch-ua",
        "\"Not/A)Brand\";v=\"8\", \"Chromium\";v=\"126\", \"Brave\";v=\"126\""
            .parse()
            .unwrap(),
    );
    headers.insert("sec-ch-ua-mobile", "?0".parse().unwrap());
    headers.insert("sec-ch-ua-platform", "\"Linux\"".parse().unwrap());
    headers.insert("sec-fetch-dest", "document".parse().unwrap());
    headers.insert("sec-fetch-mode", "navigate".parse().unwrap());
    headers.insert("sec-fetch-site", "none".parse().unwrap());
    headers.insert("sec-fetch-user", "?1".parse().unwrap());
    headers.insert("sec-gpc", "1".parse().unwrap());
    headers.insert("upgrade-insecure-requests", "1".parse().unwrap());
    headers.insert("user-agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36".parse().unwrap());

    let client = Client::builder()
        .default_headers(headers.clone())
        .redirect(reqwest::redirect::Policy::limited(5))
        .cookie_store(true)
        .build()
        .context("Failed to build HTTP client")?;
    Ok(client)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    create_dir_all(&args.folder).with_context(|| {
        format!(
            "Failed to create folder directory: {}. Error: {:?}",
            &args.folder.display(),
            std::io::Error::last_os_error()
        )
    })?;

    let cache_dir = PathBuf::from(&args.cache);
    create_dir_all(&cache_dir).with_context(|| {
        format!(
            "Failed to create cache directory: {}. Error: {:?}",
            cache_dir.display(),
            std::io::Error::last_os_error()
        )
    })?;

    let client = get_client().with_context(|| {
        format!(
            "Failed to create the reqwest client. Error: {:?}",
            std::io::Error::last_os_error()
        )
    })?;

    let page_html = fetch_or_read_page(&client, &args.url, &cache_dir).await?;

    let audio_urls = extract_options(&page_html);

    for (idx, audio_url) in audio_urls.iter().enumerate() {
        let metadata = fetch_audio_metadata(&client, audio_url, &cache_dir).await?;
        download_audio(&client, &metadata, &args.folder, idx + 1).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use std::fs::File;
    use std::io::Write;
    use tokio::fs::{create_dir_all, remove_file};

    #[tokio::test]
    async fn test_fetch_or_read_page() -> Result<()> {
        let url = "https://www.raiplaysound.it/audiolibri/itremoschettieri";
        let cache_dir = temp_dir().join("test_cache");
        create_dir_all(&cache_dir).await?;

        let client = get_client()?;

        // Pulire il file di cache se esiste
        let cache_file = cache_dir.join("itremoschettieri.html");
        if cache_file.exists() {
            remove_file(&cache_file).await?;
        }

        let result = fetch_or_read_page(&client, url, &cache_dir).await;
        assert!(result.is_ok());

        // Check that the file was cached
        let filepath = cache_dir.join("itremoschettieri.html");
        assert!(filepath.exists());

        // Pulire il file di cache
        if filepath.exists() {
            remove_file(&filepath).await?;
        }

        Ok(())
    }

    #[test]
    fn test_extract_options() {
        let html = r#"<rps-play-with-labels options='{"url": "audio/2015/06/I-tre-moschettieri---Lettura-I-2c45793e-a289-42a8-97ae-656a2a94a71f.json"}'></rps-play-with-labels>"#;
        let options = extract_options(html);
        assert_eq!(options.len(), 1);
        assert_eq!(options[0], "audio/2015/06/I-tre-moschettieri---Lettura-I-2c45793e-a289-42a8-97ae-656a2a94a71f.json");
    }

    #[tokio::test]
    async fn test_fetch_audio_metadata() -> Result<()> {
        let url = "/audio/2015/06/I-tre-moschettieri---Lettura-I-2c45793e-a289-42a8-97ae-656a2a94a71f.json";
        let cache_dir = temp_dir().join("test_cache");
        create_dir_all(&cache_dir).await?;

        // Simula la risposta JSON per il test
        let json_response = r#"
        {
            "audio": {
                "title": "I tre moschettieri - Lettura I",
                "url": "https://mediapolisvod.rai.it/relinker/relinkerServlet.htm?cont=jmC2BrdAhSIeeqqEEqual",
                "type": "audio",
                "duration": "00:19:15"
            }
        }
        "#;
        let cache_file = cache_dir
            .join("I-tre-moschettieri---Lettura-I-2c45793e-a289-42a8-97ae-656a2a94a71f.json");
        let mut file = File::create(&cache_file)?;
        file.write_all(json_response.as_bytes())?;

        let client = get_client()?;

        let metadata = fetch_audio_metadata(&client, url, &cache_dir).await?;
        assert_eq!(
            metadata.url,
            "https://mediapolisvod.rai.it/relinker/relinkerServlet.htm?cont=jmC2BrdAhSIeeqqEEqual"
        );
        assert_eq!(metadata.title, "I tre moschettieri - Lettura I");

        // Pulire il file di cache
        if cache_file.exists() {
            remove_file(&cache_file).await?;
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_download_audio() -> Result<()> {
        let metadata = AudioMetadata {
            url: "https://mediapolisvod.rai.it/relinker/relinkerServlet.htm?cont=jmC2BrdAhSIeeqqEEqual".to_string(),
            title: "Test Audio".to_string(),
        };
        let folder = temp_dir().join("test_audio");
        create_dir_all(&folder).await?;

        let client = get_client()?;

        let result = download_audio(&client, &metadata, &folder, 1).await;
        assert!(result.is_ok());

        let re = Regex::new(r"[^\w\s-]")?;
        let sanitized_title = re.replace_all(&metadata.title, "_").to_lowercase();
        let output_path = folder.join(format!("{:03} - {}.mp3", 1, sanitized_title));

        assert!(output_path.exists());

        // Pulire il file audio
        if output_path.exists() {
            remove_file(&output_path).await?;
        }

        Ok(())
    }
}
