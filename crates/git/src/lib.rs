use anyhow::{Context, Result, bail};
use git2::Repository;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

pub fn cache_path_for_url(url: &str) -> Result<PathBuf> {
    if !url.starts_with("https://") {
        bail!("only public HTTPS repository URLs are supported");
    }

    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    let base = dirs::cache_dir()
        .context("failed to locate user cache directory")?
        .join("code-explorer")
        .join("repos");
    Ok(base.join(hash))
}

pub fn clone_public_https(url: &str, reclone: bool) -> Result<PathBuf> {
    let path = cache_path_for_url(url)?;
    if path.exists() {
        if reclone {
            std::fs::remove_dir_all(&path).context("failed to remove existing clone cache")?;
        } else {
            return Ok(path);
        }
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("failed to create clone cache directory")?;
    }

    Repository::clone(url, &path).with_context(|| format!("failed to clone {url}"))?;
    Ok(path)
}
