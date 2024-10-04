use crate::api::schema::*;
use crate::config::read::*;
use crate::package::create::*;
use clap::Parser;
use custom_logger::*;
use flate2::read::GzDecoder;
use mirror_error::MirrorError;
use mirror_utils::fs_handler;
use package::signature::{create_keypair, sign_artifact, verify_artifact};
use std::fs;
use std::fs::File;
use std::process;
use tar::Archive;

mod api;
mod config;
mod package;

#[tokio::main]
async fn main() -> Result<(), MirrorError> {
    let args = Cli::parse();

    let lvl = args.loglevel.as_ref().unwrap();

    let l = match lvl.as_str() {
        "info" => Level::INFO,
        "debug" => Level::DEBUG,
        "trace" => Level::TRACE,
        _ => Level::INFO,
    };

    let log = &Logging { log_level: l };

    match &args.command {
        Some(Commands::Package {
            config_file,
            working_dir,
        }) => {
            fs_handler(format!("{}/generated", working_dir), "create_dir", None).await?;
            fs_handler(format!("{}/artifacts", working_dir), "create_dir", None).await?;
            let config = load_config(config_file.to_string()).await?;
            let sc = parse_yaml_config(config)?;
            log.debug(&format!("working-dir {}", working_dir));
            log.debug(&format!("microservices struct {:#?}", sc));
            for service in sc.spec.services.iter() {
                let res =
                    create_signed_artifact(service.name.clone(), service.project.clone()).await;
                if res.is_err() {
                    log.error(&format!(
                        "{}",
                        res.as_ref().err().unwrap().to_string().to_ascii_lowercase()
                    ));
                } else {
                    log.info(&format!(
                        "artifacts created in folder generated/{}",
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
                log.ex(&format!("  building archive for {}", service.name.clone()));
                tar_m
                    .append_dir_all(".", format!("generated/{}", service.name.clone()))
                    .unwrap();
                tar_m.finish().expect("should flush manifest contents");
                println!("\x1b[1A \x1b[38C{}", "\x1b[1;92m✓\x1b[0m");
            }
        }
        Some(Commands::Stage {
            config_file,
            working_dir,
        }) => {
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
                        let tar_gz = File::open(format!(
                            "{}/blobs/sha256/{}",
                            staging_dir,
                            service_digest.split(":").nth(1).unwrap()
                        ));
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
                        fs_handler(staging_dir, "remove_dir", None).await?;
                    }
                }
                println!("\x1b[1A \x1b[38C{}", "\x1b[1;92m✓\x1b[0m");
            }
        }
        Some(Commands::CreateReferralManifest {
            name,
            referral_url_digest,
            referral_size,
            format,
        }) => {
            let res = create_referral_manifest(
                name.to_string(),
                referral_url_digest.to_string(),
                *referral_size,
                format.to_string(),
            )
            .await;
            if res.is_err() {
                log.error(&format!(
                    "{}",
                    res.as_ref().err().unwrap().to_string().to_ascii_lowercase()
                ));
            } else {
                log.info(&format!("created signed manifest for {}", name));
            }
        }
        Some(Commands::Keypair {}) => {
            create_keypair().await?;
        }
        Some(Commands::Sign { artifact }) => {
            let name = artifact.split("/").last().unwrap();
            let res = sign_artifact(name.to_string(), artifact.to_string()).await;
            if res.is_err() {
                log.error(&format!(
                    "{:#?}",
                    res.err().as_ref().unwrap().to_string().to_lowercase()
                ));
            }
            log.info(&format!("artifact {} successfully signed", name));
        }
        Some(Commands::Verify { artifact }) => {
            let name = artifact.split("/").last().unwrap();
            let res = verify_artifact(name.to_string(), artifact.to_string()).await;
            if res.is_err() {
                log.error(&format!(
                    "{:#?}",
                    res.as_ref().err().unwrap().to_string().to_lowercase()
                ));
            }
            match res.as_ref().unwrap() {
                true => log.info(&format!("artifact {} is trusted", name)),
                false => log.warn(&format!("artitact {} is not trusted", name)),
            }
        }
        None => {
            log.error("sub command not recognized, use --help to get list of cli options");
            process::exit(1);
        }
    }
    Ok(())
}
