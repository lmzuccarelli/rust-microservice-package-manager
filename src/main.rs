use crate::api::schema::*;
use crate::package::create::*;
use crate::package::signature::{create_keypair, sign_artifact, verify_artifact};
use crate::websocket::client::*;
use crate::websocket::server::*;
use clap::Parser;
use custom_logger::*;
use mirror_error::MirrorError;
use std::process;

mod api;
mod config;
mod package;
mod websocket;
mod workflow;

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
    let mode: &str;
    let log = &Logging { log_level: l };
    if args.mode.is_none() {
        mode = "none";
    } else {
        mode = args.mode.as_ref().unwrap()
    }
    match mode {
        "worker" => {
            let res = start_client(log).await;
            if res.is_err() {
                log.error(&format!(
                    "worker {}",
                    res.err().unwrap().to_string().to_lowercase(),
                ));
                process::exit(1);
            }
        }
        "controller" => {
            let res = start_server(log).await;
            if res.is_err() {
                log.error(&format!(
                    "controller {}",
                    res.err().unwrap().to_string().to_lowercase(),
                ));
                process::exit(1);
            }
        }
        _ => match &args.command {
            Some(Commands::Package {
                config_file,
                working_dir,
                skip_tls_verify,
            }) => {
                let api_params = APIParameters {
                    command: "package".to_string(),
                    node: "local".to_string(),
                    service: "all".to_string(),
                    config_file: config_file.clone(),
                    working_dir: working_dir.clone(),
                    from_registry: true,
                    skip_tls_verify: *skip_tls_verify,
                };
                let message = serde_json::to_string(&api_params).unwrap();
                let res = send_message(log, message).await;
                if res.is_err() {
                    log.error(&format!(
                        "package {}",
                        res.err().unwrap().to_string().to_lowercase(),
                    ));
                } else {
                    log.info("package message sent")
                }
            }
            Some(Commands::Stage {
                node,
                config_file,
                working_dir,
                from_registry,
                skip_tls_verify,
            }) => {
                let api_params = APIParameters {
                    command: "stage".to_string(),
                    node: node.clone(),
                    service: "all".to_string(),
                    config_file: config_file.clone(),
                    working_dir: working_dir.clone(),
                    from_registry: *from_registry,
                    skip_tls_verify: *skip_tls_verify,
                };
                let message = serde_json::to_string(&api_params).unwrap();
                let res = send_message(log, message).await;
                if res.is_err() {
                    log.error(&format!(
                        "send message {}",
                        res.err().unwrap().to_string().to_lowercase()
                    ));
                } else {
                    log.info("stage message sent")
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
                log.ex("keypair successfully created")
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
            Some(Commands::Start {
                node,
                service,
                config_file,
                working_dir,
                skip_tls_verify,
            }) => {
                let api_params = APIParameters {
                    command: "start".to_string(),
                    node: node.clone(),
                    service: service.clone(),
                    config_file: config_file.clone(),
                    working_dir: working_dir.clone(),
                    from_registry: true,
                    skip_tls_verify: *skip_tls_verify,
                };
                let message = serde_json::to_string(&api_params).unwrap();
                let res = send_message(log, message).await;
                if res.is_err() {
                    log.error(&format!(
                        "send message {}",
                        res.err().unwrap().to_string().to_lowercase()
                    ));
                } else {
                    log.info("start message sent");
                }
            }
            Some(Commands::Stop {
                node,
                service,
                config_file,
                working_dir,
                skip_tls_verify,
            }) => {
                let api_params = APIParameters {
                    command: "stop".to_string(),
                    node: node.clone(),
                    service: service.clone(),
                    config_file: config_file.clone(),
                    working_dir: working_dir.clone(),
                    from_registry: true,
                    skip_tls_verify: *skip_tls_verify,
                };
                let message = serde_json::to_string(&api_params).unwrap();
                let res = send_message(log, message).await;
                if res.is_err() {
                    log.error(&format!(
                        "send message {}",
                        res.err().unwrap().to_string().to_lowercase()
                    ));
                } else {
                    log.info("stop message sent");
                }
            }
            Some(Commands::List {}) => {
                let api_params = APIParameters {
                    command: "list".to_string(),
                    node: "all".to_string(),
                    service: "".to_string(),
                    config_file: "".to_string(),
                    working_dir: "".to_string(),
                    from_registry: true,
                    skip_tls_verify: true,
                };
                let message = serde_json::to_string(&api_params).unwrap();
                let res = send_message(log, message).await;
                if res.is_err() {
                    log.error(&format!(
                        "send message {}",
                        res.err().unwrap().to_string().to_lowercase()
                    ));
                } else {
                    log.info("list message sent");
                }
            }
            None => {
                log.error("sub command not recognized, use --help to get list of cli options");
                process::exit(1);
            }
        },
    }
    Ok(())
}
