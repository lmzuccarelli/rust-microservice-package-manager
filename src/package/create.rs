use crate::api::schema::{Layer, Manifest, OCIIndex};
use crate::{Annotations, SignatureJson};
use base64::prelude::*;
use flate2::write::GzEncoder;
use flate2::Compression;
use mirror_error::MirrorError;
use mirror_utils::fs_handler;
use sha256::digest;
use std::fs;
use std::fs::File;
use std::io::Read;

// first pick up the binar file and create a tar.gz
pub async fn create_signed_artifact(name: String, path: String) -> Result<(), MirrorError> {
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
    create_oci_manifest(name.clone(), hash, buf.len()).await?;
    Ok(())
}

pub async fn create_oci_manifest(
    name: String,
    ms_hash: String,
    ms_size: usize,
) -> Result<(), MirrorError> {
    // create the referenced image manifest
    let empty = "  ".to_string();
    let hash = digest(empty.as_bytes());
    let cfg_layer = Layer {
        media_type: "application/vnd.oci.image.config.v1+json".to_string(),
        digest: format!("sha256:{}", hash.clone()),
        size: 2,
        annotations: None,
    };
    let blob_cfg = format!("generated/{}/blobs/sha256/{}", name, hash);
    fs_handler(blob_cfg, "write", Some("  ".to_string())).await?;

    let ms_layer = Layer {
        media_type: "application/vnd.oci.image.layer.v1.tar+gzip".to_string(),
        digest: format!("sha256:{}", ms_hash),
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
        digest: format!("sha256:{}", hash_json),
        size: manifest_json.len() as i64,
        annotations: None,
    };
    let vec_manifests = vec![layer];
    let index = OCIIndex {
        schema_version: 2,
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

pub async fn create_referral_manifest(
    name: String,
    referral_url_digest: String,
    referral_size: i64,
) -> Result<(), MirrorError> {
    fs_handler(format!("generated/{}/signature", name), "create_dir", None).await?;
    let mut artifact_buf = vec![];
    let res_file = File::open(format!(".ssh/{}-signature", name.clone()));
    if res_file.is_err() {
        let err = MirrorError::new(&format!(
            "opening artifact file {}",
            res_file.err().unwrap().to_string().to_lowercase()
        ));
        return Err(err);
    }
    let res_buf = res_file.unwrap().read_to_end(&mut artifact_buf);
    if res_buf.is_err() {
        let err = MirrorError::new(&format!(
            "reading in buffer {}",
            res_buf.err().unwrap().to_string().to_lowercase()
        ));
        return Err(err);
    }
    // base64 encode data signed file
    let encoded = BASE64_STANDARD.encode(artifact_buf);
    let sig_json = SignatureJson {
        artifact: referral_url_digest.clone(),
        signature: encoded.clone(),
    };
    let sig_json_contents = serde_json::to_string(&sig_json).unwrap();
    let hash_sig_json = digest(&sig_json_contents.clone());
    fs_handler(
        format!("generated/{}/signature/{}", name, hash_sig_json),
        "write",
        Some(sig_json_contents.clone()),
    )
    .await?;
    // create the layer
    let sig_annotations = Annotations {
        image_title: Some(hash_sig_json.clone()),
        image_created: None,
    };
    let sig_layer = Layer {
        digest: format!("sha256:{}", hash_sig_json.clone()),
        media_type: "application/json".to_string(),
        size: sig_json_contents.clone().len() as i64,
        annotations: Some(sig_annotations),
    };
    // create the referenced image manifest
    let empty = "  ".to_string();
    let hash = digest(empty.as_bytes());
    let cfg_layer = Layer {
        media_type: "application/vnd.oci.image.empty.v1+json".to_string(),
        digest: format!("sha256:{}", hash.clone()),
        size: 2,
        annotations: None,
    };
    fs_handler(
        format!("generated/{}/signature/{}", name, hash),
        "write",
        Some(empty),
    )
    .await?;

    // assemble to manifest.json (dockerv2 format)
    let subject_ref = Layer {
        media_type: "application/vnd.oci.image.manifest.v1+json".to_string(),
        digest: referral_url_digest.split("@").nth(1).unwrap().to_string(),
        size: referral_size,
        annotations: None,
    };
    let vec_layers = vec![sig_layer];
    let manifest = Manifest {
        schema_version: Some(2),
        artifact_type: Some("application/application/vnd.example.signature.v1+json".to_string()),
        media_type: Some("application/vnd.oci.image.manifest.v1+json".to_string()),
        config: Some(cfg_layer.clone()),
        layers: Some(vec_layers.clone()),
        digest: None,
        platform: None,
        size: None,
        subject: Some(subject_ref.clone()),
    };

    let manifest_json = serde_json::to_string(&manifest).unwrap();
    let manifest_file = format!("generated/{}/signature/manifest.json", name.clone());
    fs_handler(manifest_file, "write", Some(manifest_json.clone())).await?;

    Ok(())
}
