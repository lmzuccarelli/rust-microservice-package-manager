use crate::api::schema::{Layer, Manifest, ManifestPlatform, OCIIndex};
use crate::{Annotations, SignatureJson};
use base64::prelude::*;
use flate2::write::GzEncoder;
use flate2::Compression;
use mirror_error::MirrorError;
use mirror_utils::fs_handler;
use sha2::{Digest, Sha256};
use sha256::digest;
use std::fs::File;
use std::io;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::{fs, usize};

// first pick up the binary file and create a tar.gz
pub async fn create_signed_artifact(name: String, path: String) -> Result<(), MirrorError> {
    let tar_gz_file = format!("generated/{}.tar.gz", name.clone());
    let tar = File::create(tar_gz_file.clone()).unwrap();
    let enc = GzEncoder::new(tar, Compression::default());
    let mut tar_file = tar::Builder::new(enc);
    let res_binary = File::open(format!("{}/{}", path, name.clone()));
    if res_binary.is_err() {
        let err = MirrorError::new(&format!(
            "reading binary microservice (maybe needs to be compiled ?) {}",
            res_binary.err().unwrap().to_string().to_lowercase()
        ));
        return Err(err);
    }
    //let mut binary = res_binary.unwrap();
    tar_file.append_dir_all(".", path.clone()).unwrap();
    tar_file.into_inner().unwrap().finish().unwrap();

    let mut file = std::fs::File::open(tar_gz_file.clone()).unwrap();
    let mut hasher = Sha256::new();
    let res = io::copy(&mut file, &mut hasher);
    if res.is_err() {
        let err = MirrorError::new(&format!(
            "creating sh256 hash {}",
            res.err().unwrap().to_string().to_lowercase()
        ));
        return Err(err);
    }
    let digest = format!("{:x}", hasher.finalize());
    let blobs_path = format!("generated/{}/blobs/sha256", name.clone());
    fs_handler(blobs_path.clone(), "create_dir", None).await?;
    let rename = fs::rename(
        tar_gz_file,
        format!("{}/{}", blobs_path.clone(), digest.clone()),
    );
    if rename.is_err() {
        let err = MirrorError::new(&format!(
            "renaming file to blob {}",
            rename.err().unwrap().to_string().to_lowercase()
        ));
        return Err(err);
    }
    let metadata = fs::metadata(format!("{}/{}", blobs_path.clone(), digest.clone())).unwrap();
    create_oci_manifest(name.clone(), digest, metadata.size() as usize).await?;
    Ok(())
}

pub async fn create_oci_manifest(
    name: String,
    ms_hash: String,
    ms_size: usize,
) -> Result<(), MirrorError> {
    // create the referenced image manifest
    let cfg = fs_handler("templates/config-simple.json".to_string(), "read", None).await?;
    let hash = digest(cfg.as_bytes());
    let cfg_layer = Layer {
        media_type: "application/vnd.oci.image.config.v1+json".to_string(),
        digest: format!("sha256:{}", hash.clone()),
        size: cfg.len() as i64,
        annotations: None,
    };
    let blob_cfg = format!("generated/{}/blobs/sha256/{}", name, hash);
    fs_handler(blob_cfg, "write", Some(cfg.to_string())).await?;

    let ms_layer = Layer {
        media_type: "application/vnd.oci.image.layer.v1.tar+gzip".to_string(),
        digest: format!("sha256:{}", ms_hash),
        size: ms_size as i64,
        annotations: None,
    };
    let vec_layers = vec![ms_layer];
    let mnfst_platform = ManifestPlatform {
        architecture: "amd64".to_string(),
        os: "linux".to_string(),
    };

    let manifest = Manifest {
        schema_version: Some(2),
        artifact_type: None,
        media_type: Some("application/vnd.oci.image.manifest.v1+json".to_string()),
        config: Some(cfg_layer),
        layers: Some(vec_layers),
        digest: None,
        platform: Some(mnfst_platform),
        size: None,
        subject: None,
    };

    let manifest_json = serde_json::to_string(&manifest).unwrap();
    let hash_json = digest(&manifest_json);
    let manifest_blob_json = format!("generated/{}/blobs/sha256/{}", name, hash_json);
    fs_handler(manifest_blob_json, "write", Some(manifest_json.clone())).await?;

    // create oci index first
    let index = Layer {
        media_type: "application/vnd.oci.image.manifest.v1+json".to_string(),
        digest: format!("sha256:{}", hash_json),
        size: manifest_json.len() as i64,
        annotations: None,
    };
    let vec_manifests = vec![index];
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

// referral manifest (oci format, used to create signature layer)
pub async fn create_referral_manifest(
    name: String,
    referral_url_digest: String,
    referral_size: i64,
    format: String,
) -> Result<(), MirrorError> {
    if format == "dockerv2" {
        fs_handler(format!("generated/{}/signature", name), "create_dir", None).await?;
    } else {
        fs_handler(
            format!("generated/{}/signature/blobs/sha256", name),
            "create_dir",
            None,
        )
        .await?;
    }
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
    if format == "dockerv2" {
        fs_handler(
            format!("generated/{}/signature/{}", name, hash_sig_json),
            "write",
            Some(sig_json_contents.clone()),
        )
        .await?;
    } else {
        fs_handler(
            format!(
                "generated/{}/signature/blobs/sha256/{}",
                name, hash_sig_json
            ),
            "write",
            Some(sig_json_contents.clone()),
        )
        .await?;
    }
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
    if format == "dockerv2" {
        fs_handler(
            format!("generated/{}/signature/{}", name, hash),
            "write",
            Some(empty),
        )
        .await?;
    } else {
        fs_handler(
            format!("generated/{}/signature/blobs/sha256/{}", name, hash),
            "write",
            Some(empty),
        )
        .await?;
    }

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
        artifact_type: Some("application/vnd.example.signature.v1+json".to_string()),
        media_type: Some("application/vnd.oci.image.manifest.v1+json".to_string()),
        config: Some(cfg_layer.clone()),
        layers: Some(vec_layers.clone()),
        digest: None,
        platform: None,
        size: None,
        subject: Some(subject_ref.clone()),
    };
    let manifest_json = serde_json::to_string(&manifest).unwrap();
    if format == "dockerv2" {
        let manifest_file = format!("generated/{}/signature/manifest.json", name.clone());
        fs_handler(manifest_file, "write", Some(manifest_json.clone())).await?;
    } else {
        let hash = digest(&manifest_json);
        let manifest_file = format!("generated/{}/signature/blobs/sha256/{}", name.clone(), hash);
        fs_handler(manifest_file, "write", Some(manifest_json.clone())).await?;
        // finally create an index.json
        let layer = Layer {
            media_type: "application/vnd.oci.image.manifest.v1+json".to_string(),
            digest: format!("sha256:{}", hash),
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
            format!("generated/{}/signature/index.json", name),
            "write",
            Some(index_json.unwrap()),
        )
        .await?;
    }
    Ok(())
}
