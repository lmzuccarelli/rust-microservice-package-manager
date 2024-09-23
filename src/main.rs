use crate::api::schema::*;
use crate::config::read::*;
use crate::package::create::*;
use clap::Parser;
use custom_logger::*;
use mirror_error::MirrorError;
use mirror_utils::fs_handler;
use package::signature::{create_keypair, sign_artifact, verify_artifact};
use std::process;

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
                    log.info(&format!("artifacts created in generated/{}", service.name));
                    log.info(&format!("completed packaging for {}", service.name));
                }
            }
        }
        Some(Commands::CreateManifest {
            name,
            referral_url_digest,
            referral_size,
        }) => {
            let res = create_referral_manifest(
                name.to_string(),
                referral_url_digest.to_string(),
                *referral_size,
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
            log.error("sub command not recognized");
            process::exit(1);
        }
    }
    Ok(())
}
