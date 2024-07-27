use std::path::PathBuf;

use cached::proc_macro::cached;
use rocket::fs::NamedFile;
use tokio::fs::{create_dir_all, read_to_string, write};
use tokio::try_join;

#[cached(unbound)]
async fn artifact_cache_dir() -> PathBuf {
    xdg::BaseDirectories::with_prefix("deposit-box")
        .unwrap()
        .get_cache_home()
        .join("nightlies")
}

pub(super) async fn get_cached_artifact_run_id(product: &str, artifact: &str) -> String {
    let dir = artifact_cache_dir().await;
    let run_id_path = dir.join(product).join(format!("{artifact}.runid"));
    let artifact_path = dir.join(format!("{artifact}.zip"));
    if run_id_path.exists() && artifact_path.exists() {
        read_to_string(run_id_path).await.unwrap_or_default()
    } else {
        String::new()
    }
}

pub(super) async fn store_cached_artifact_run(
    product: &str,
    artifact: &str,
    run_id: String,
    binartifact: bytes::Bytes,
) -> Result<(), tokio::io::Error> {
    let dir = artifact_cache_dir().await;
    let prod_dir = dir.join(product);
    create_dir_all(&prod_dir).await.unwrap();
    let run_id_path = dir.join(product).join(format!("{artifact}.runid"));
    let artifact_path = prod_dir.join(format!("{artifact}.zip"));
    let write_run_id = write(run_id_path, run_id);
    let write_artifact = write(artifact_path, binartifact);
    try_join!(write_run_id, write_artifact)?;
    Ok(())
}

pub(super) async fn get_cached_artifact(
    product: &str,
    artifact: &str,
) -> Result<NamedFile, tokio::io::Error> {
    let dir = artifact_cache_dir().await;
    let prod_dir = dir.join(product);
    create_dir_all(&prod_dir).await.unwrap();
    let filename = format!("{artifact}.zip");
    let artifact_path = prod_dir.join(&filename);
    NamedFile::open(&artifact_path).await
}
