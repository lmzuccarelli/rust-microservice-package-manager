use crate::api::schema::*;
use crate::config::read::*;
use crate::package::create::*;
use crate::package::signature::*;
use custom_logger::*;
use flate2::read::GzDecoder;
use mirror_auth::{get_token, ImplTokenInterface};
use mirror_copy::{
    DownloadImageInterface, ImplDownloadImageInterface, ImplUploadImageInterface, ManifestType,
    UploadImageInterface,
};
use mirror_error::MirrorError;
use mirror_utils::{fs_handler, ImageReference};
use std::fs;
use std::fs::File;
use std::process;
use tar::Archive;

pub async fn package(
    log: &Logging,
    working_dir: String,
    config_file: String,
    skip_tls_verify: bool,
) -> Result<(), MirrorError> {
    fs_handler(format!("{}/generated", working_dir), "remove_dir", None).await?;
    fs_handler(format!("{}/generated", working_dir), "create_dir", None).await?;
    fs_handler(format!("{}/artifacts", working_dir), "create_dir", None).await?;
    let config = load_config(config_file.to_string()).await?;
    let sc = parse_yaml_config(config)?;
    log.debug(&format!("working-dir {}", working_dir));
    log.debug(&format!("microservices struct {:#?}", sc));
    for service in sc.spec.services.iter() {
        // first sign each artifact
        let res = sign_artifact(
            service.name.clone(),
            format!("{}/{}", service.binary_path.clone(), service.name.clone()),
        )
        .await;
        if res.is_err() {
            log.error(&format!(
                "[package] signing binary {} {}",
                service.name.clone(),
                res.err().as_ref().unwrap().to_string().to_lowercase()
            ));
            process::exit(1);
        }
        let res = create_signed_artifact(service.name.clone(), service.binary_path.clone()).await;
        if res.is_err() {
            log.error(&format!(
                "[package] creating package {} {}",
                service.name.clone(),
                res.as_ref().err().unwrap().to_string().to_ascii_lowercase()
            ));
            process::exit(1);
        } else {
            log.info(&format!(
                "[package] artifacts created in folder generated/{}",
                service.name
            ));
        }
        // tar each oci image
        let tar_file = File::create(format!(
            "{}/artifacts/{}.pkg",
            working_dir,
            service.name.clone()
        ))
        .unwrap();

        let mut tar_m = tar::Builder::new(tar_file);
        log.ex(&format!(
            "  building artifacts for {}",
            service.name.clone()
        ));
        tar_m
            .append_dir_all(".", format!("generated/{}", service.name.clone()))
            .unwrap();
        tar_m.finish().expect("should flush manifest contents");

        let parts = service.registry.split("/").collect::<Vec<&str>>();
        let (name, version) = parts[3].split_once(":").unwrap();
        let img_ref = ImageReference {
            registry: parts[0].to_string(),
            namespace: format!("{}/{}", parts[1], parts[2]),
            name: name.to_string(),
            version: version.to_string(),
        };
        let impl_t = ImplTokenInterface {};
        let impl_u = ImplUploadImageInterface {};
        let local_token = get_token(
            impl_t,
            log,
            img_ref.registry.clone(),
            format!("{}/{}", img_ref.namespace.clone(), img_ref.name.clone()),
            !skip_tls_verify,
        )
        .await?;

        let paths = fs::read_dir(format!(
            "{}/generated/{}/blobs/sha256/",
            working_dir, service.name
        ))
        .unwrap();
        for path in paths {
            let digest = path
                .unwrap()
                .path()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            let req_blobs = impl_u
                .process_blob(
                    log,
                    img_ref.registry.clone(),
                    format!("{}/{}", img_ref.namespace, img_ref.name),
                    format!("{}/generated/{}/blobs/sha256/", working_dir, service.name),
                    true,
                    digest.clone(),
                    local_token.clone(),
                )
                .await;
            if req_blobs.is_err() {
                println!("\x1b[1A \x1b[38C{}", "\x1b[1;91m✗\x1b[0m");
                log.error(&format!(
                    "{}",
                    req_blobs.err().as_ref().unwrap().to_string().to_lowercase()
                ));
                process::exit(1);
            }
        }
        let data = fs_handler(
            format!("{}/generated/{}/index.json", working_dir, service.name),
            "read",
            None,
        )
        .await?;
        let res_index = serde_json::from_str(&data);
        if res_index.is_err() {
            println!("\x1b[1A \x1b[38C{}", "\x1b[1;91m✗\x1b[0m");
            log.error(&format!(
                "parsing index.json {}",
                res_index.err().as_ref().unwrap().to_string().to_lowercase()
            ));
            process::exit(1);
        }
        let index: OCIIndex = res_index.unwrap();
        let digest = index.manifests[0].digest.clone();
        // read the manifest
        let mnfst = fs_handler(
            format!(
                "{}/generated/{}/blobs/sha256/{}",
                working_dir,
                service.name,
                digest.split(":").nth(1).unwrap()
            ),
            "read",
            None,
        )
        .await?;
        let req_mfst = impl_u
            .process_manifest_string(
                log,
                img_ref.registry.clone(),
                format!("{}/{}", img_ref.namespace, img_ref.name),
                mnfst.clone(),
                ManifestType::Oci,
                img_ref.version,
                local_token.clone(),
            )
            .await;
        if req_mfst.is_err() {
            println!("\x1b[1A \x1b[38C{}", "\x1b[1;91m✗\x1b[0m");
            log.error(&format!(
                "{}",
                req_mfst.err().as_ref().unwrap().to_string().to_lowercase()
            ));
            process::exit(1);
        }
        println!("\x1b[1A \x1b[38C{}", "\x1b[1;92m✓\x1b[0m");
    }
    Ok(())
}

pub async fn stage(
    log: &Logging,
    from_registry: bool,
    working_dir: String,
    config_file: String,
    skip_tls_verify: bool,
) -> Result<(), MirrorError> {
    log.trace(&format!("from-registry {}", from_registry));
    let config = load_config(config_file.to_string()).await?;
    let sc = parse_yaml_config(config)?;
    log.debug(&format!("working-dir {}", working_dir));
    log.debug(&format!("microservices struct {:#?}", sc));
    for service in sc.spec.services.iter() {
        log.ex(&format!("  staging for service {}", service.name.clone()));
        let staging_dir = format!("{}/staging/{}", working_dir, service.name.clone());
        fs_handler(staging_dir.clone(), "create_dir", None).await?;
        let ms_dir = format!("{}/microservices/{}", working_dir, service.name.clone());
        fs_handler(ms_dir.clone(), "create_dir", None).await?;
        if !from_registry {
            let data = std::fs::File::open(format!(
                "{}/artifacts/{}.pkg",
                working_dir,
                service.name.clone()
            ));
            let mut archive = Archive::new(data.unwrap());
            let res = archive.unpack(staging_dir.clone());
            if res.is_err() {
                println!("\x1b[1A \x1b[38C{}", "\x1b[1;91m✗\x1b[0m");
                log.error(&format!(
                    "[staging] untar service package {}",
                    res.as_ref().err().unwrap().to_string().to_ascii_lowercase()
                ));
                process::exit(1);
            }
        } else {
            // pull artifacts from registry
            let parts = service.registry.split("/").collect::<Vec<&str>>();
            let (name, version) = parts[3].split_once(":").unwrap();

            let img_ref = ImageReference {
                registry: parts[0].to_string(),
                namespace: format!("{}/{}", parts[1], parts[2]),
                name: name.to_string(),
                version: version.to_string(),
            };
            let impl_t = ImplTokenInterface {};
            let local_token = get_token(
                impl_t,
                log,
                img_ref.registry.clone(),
                format!("{}/{}", img_ref.namespace.clone(), img_ref.name.clone()),
                !skip_tls_verify,
            )
            .await?;

            let impl_d = ImplDownloadImageInterface {};
            let url = format!(
                "https://{}/v2/{}/{}/manifests/{}",
                img_ref.registry, img_ref.namespace, img_ref.name, img_ref.version
            );
            let manifest = impl_d
                .get_manifest(url.clone(), local_token.clone())
                .await?;

            fs_handler(
                format!("{}/index.json", staging_dir.clone()),
                "write",
                Some(manifest.clone()),
            )
            .await?;

            let res_json = serde_json::from_str(&manifest);
            log.trace(&format!("index.json {}", manifest));
            if res_json.is_err() {
                println!("\x1b[1A \x1b[38C{}", "\x1b[1;91m✗\x1b[0m");
                log.error(&format!(
                    "[staging] parsing index.json {}",
                    res_json
                        .as_ref()
                        .err()
                        .unwrap()
                        .to_string()
                        .to_ascii_lowercase()
                ));
                process::exit(1);
            }
            let oci_index: Manifest = res_json.unwrap();
            let blob_sum_sha = oci_index.layers.unwrap()[0].digest.clone();
            let blob_sum = blob_sum_sha.split(":").nth(1).unwrap();
            let blobs_dir = format!("{}/blobs/sha256/", staging_dir);
            impl_d
                .get_blob(
                    log,
                    blobs_dir.clone(),
                    url,
                    local_token,
                    false,
                    blob_sum_sha.to_string(),
                )
                .await?;
            fs_handler(
                format!("{}/{}", blobs_dir, &blob_sum[..2]),
                "remove_dir",
                None,
            )
            .await?;
        }

        let data = fs::read_to_string(staging_dir.clone() + &"/index.json");
        if data.is_err() {
            println!("\x1b[1A \x1b[38C{}", "\x1b[1;91m✗\x1b[0m");
            log.error(&format!(
                "[staging] reading index.json {}",
                data.as_ref()
                    .err()
                    .unwrap()
                    .to_string()
                    .to_ascii_lowercase()
            ));
            process::exit(1);
        }
        let res_json = serde_json::from_str(&data.unwrap());
        if res_json.is_ok() {
            let oci_index: OCIIndex = res_json.unwrap();
            let digest = oci_index.manifests[0].digest.clone();
            let manifest_data = fs::read_to_string(format!(
                "{}/blobs/sha256/{}",
                staging_dir,
                digest.split(":").nth(1).unwrap()
            ));
            if manifest_data.is_err() {
                println!("\x1b[1A \x1b[38C{}", "\x1b[1;91m✗\x1b[0m");
                log.error(&format!(
                    "[staging] reading manifest {}",
                    manifest_data
                        .as_ref()
                        .err()
                        .unwrap()
                        .to_string()
                        .to_ascii_lowercase()
                ));
                process::exit(1);
            }
            let res_manifest_json = serde_json::from_str(&manifest_data.unwrap());
            if res_manifest_json.is_ok() {
                let manifest: Manifest = res_manifest_json.unwrap();
                // only interested in the tar.gz layer
                let service_digest = manifest.layers.unwrap()[0].digest.clone();
                let blob_file = match from_registry {
                    true => format!(
                        "{}/blobs/sha256/{}/{}",
                        staging_dir,
                        &service_digest[..2],
                        service_digest
                    ),
                    false => format!("{}/blobs/sha256/{}/", staging_dir, service_digest),
                };
                let tar_gz = File::open(blob_file);
                let tar = GzDecoder::new(tar_gz.unwrap());
                let mut archive = Archive::new(tar);
                let res_untar = archive.unpack(ms_dir);
                if res_untar.is_err() {
                    println!("\x1b[1A \x1b[38C{}", "\x1b[1;91m✗\x1b[0m");
                    log.error(&format!(
                        "[staging] untar service binary {}",
                        res_untar
                            .as_ref()
                            .err()
                            .unwrap()
                            .to_string()
                            .to_ascii_lowercase()
                    ));
                    process::exit(1);
                }
                //fs_handler(format!("{}/staging", working_dir), "remove_dir", None).await?;
            }
        }
        println!("\x1b[1A \x1b[38C{}", "\x1b[1;92m✓\x1b[0m");
    }
    Ok(())
}
