use crate::api::schema::{Layer, Manifest, OCIIndex};
use flate2::write::GzEncoder;
use flate2::Compression;
use mirror_error::MirrorError;
use mirror_utils::fs_handler;
use sha256::digest;
use std::fs;
use std::fs::File;
use std::io::Read;

// first pick up the binar file and create a tar.gz
pub async fn create_tar_gz(name: String, path: String) -> Result<(), MirrorError> {
    let tar_gz_file = format!("generated/{}.tar.gz", name.clone());
    let tar = File::create(tar_gz_file.clone()).unwrap();
    let enc = GzEncoder::new(tar, Compression::default());
    let mut a = tar::Builder::new(enc);
    let mut binary = File::open(format!("{}/{}", path, name.clone())).unwrap();
    a.append_file(name.clone(), &mut binary).unwrap();
    let mut buf = vec![];
    let mut tgz_file = File::open(tar_gz_file.clone()).unwrap();
    let res_r = tgz_file.read_to_end(&mut buf);
    if res_r.is_err() {
        let err = MirrorError::new(&format!(
            "reading tar.gz file for hashing {}",
            res_r.err().unwrap().to_string().to_lowercase()
        ));
        return Err(err);
    }
    let hash = digest(&buf);
    let blobs_path = format!("generated/{}/blobs/sha256", name.clone());
    fs_handler(blobs_path.clone(), "create_dir", None).await?;
    let res_w = fs::write(
        format!("generated/{}/blobs/sha256/{}", name, hash),
        buf.clone(),
    );
    if res_w.is_err() {
        let err = MirrorError::new(&format!(
            "writing blob {}",
            res_w.err().unwrap().to_string().to_lowercase()
        ));
        return Err(err);
    }
    fs_handler(tar_gz_file, "remove_file", None).await?;
    create_manifests(name.clone(), hash, buf.len()).await?;
    Ok(())
}

pub async fn create_manifests(
    name: String,
    ms_hash: String,
    ms_size: usize,
) -> Result<(), MirrorError> {
    // create the referenced image manifest
    let empty = "  ".to_string();
    let hash = digest(empty.as_bytes());
    let cfg_layer = Layer {
        media_type: "application/vnd.oci.empty.v1+json".to_string(),
        digest: hash.clone(),
        size: 2,
        annotations: None,
    };
    let blob_cfg = format!("generated/{}/blobs/sha256/{}", name, hash);
    fs_handler(blob_cfg, "write", Some("  ".to_string())).await?;

    let ms_layer = Layer {
        media_type: "application/vnd.oci.image.layer.v1.tar+gzip".to_string(),
        digest: ms_hash,
        size: ms_size as i64,
        annotations: None,
    };
    let vec_layers = vec![ms_layer];
    let manifest = Manifest {
        schema_version: Some(2),
        artifact_type: None,
        media_type: Some("application/vnd.oci.image.config.v1+json".to_string()),
        config: Some(cfg_layer),
        layers: Some(vec_layers),
        digest: None,
        platform: None,
        size: None,
        subject: None,
    };

    let manifest_json = serde_json::to_string(&manifest).unwrap();
    let hash_json = digest(&manifest_json);
    let blobs_json = format!("generated/{}/blobs/sha256/{}", name, hash_json);
    fs_handler(blobs_json, "write", Some(manifest_json.clone())).await?;

    // create oci index first
    let layer = Layer {
        media_type: "application/vnd.oci.image.manifest.v1+json".to_string(),
        digest: hash_json,
        size: manifest_json.len() as i64,
        annotations: None,
    };
    let vec_manifests = vec![layer];
    let index = OCIIndex {
        schema_version: "2".to_string(),
        manifests: vec_manifests.clone(),
    };
    let index_json = serde_json::to_string(&index);
    fs_handler(
        format!("generated/{}/index.json", name),
        "write",
        Some(index_json.unwrap()),
    )
    .await?;

    Ok(())
}
