use crate::api::schema::*;
use crate::package::create::*;
use crate::package::signature::{create_keypair, sign_artifact, verify_artifact};
use crate::websocket::client::*;
use crate::websocket::server::*;
use clap::Parser;
use custom_logger::*;
use mirror_error::MirrorError;
use remote::process::{remote_execute, remote_upload};
use std::process;
use workflow::handler;

mod api;
mod command;
mod common;
mod config;
mod network;
mod package;
mod remote;
mod websocket;
mod workflow;

#[tokio::main]
async fn main() -> Result<(), MirrorError> {
    let args = Cli::parse();

    let lvl = args.loglevel.as_ref().unwrap();
    let l = match lvl.as_str() {
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Info,
    };
    let log = Logging::new().with_level(l);
    log.init().expect("log should initialize");

    let mode: &str;
    if args.mode.is_none() {
        mode = "none";
    } else {
        mode = args.mode.as_ref().unwrap()
    }
    let server_ip = match args.server_ip {
        None => "127.0.0.1".to_string(),
        Some(ip) => ip,
    };
    match mode {
        "worker" => {
            let res = start_client(server_ip).await;
            if res.is_err() {
                error!("worker {}", res.err().unwrap().to_string().to_lowercase(),);
                process::exit(1);
            }
        }
        "controller" => {
            let res = start_server(server_ip).await;
            if res.is_err() {
                error!(
                    "controller {}",
                    res.err().unwrap().to_string().to_lowercase(),
                );
                process::exit(1);
            }
        }
        _ => match &args.command {
            Some(Commands::Package {
                config_file,
                working_dir,
                skip_tls_verify,
            }) => {
                let res = handler::package(working_dir, config_file, skip_tls_verify).await;
                if res.is_err() {
                    error!("package {}", res.err().unwrap().to_string().to_lowercase());
                    process::exit(1);
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
                    config_file: Some(config_file.clone()),
                    working_dir: Some(working_dir.clone()),
                    from_registry: Some(*from_registry),
                    skip_tls_verify: Some(*skip_tls_verify),
                    ip: None,
                    subnet: None,
                };
                let message = serde_json::to_string(&api_params).unwrap();
                let res = send_message(message, server_ip).await;
                if res.is_err() {
                    error!(
                        "send message {}",
                        res.err().unwrap().to_string().to_lowercase()
                    );
                } else {
                    info!("stage message sent")
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
                    error!(
                        "{}",
                        res.as_ref().err().unwrap().to_string().to_ascii_lowercase()
                    );
                } else {
                    info!("created signed manifest for {}", name);
                }
            }
            Some(Commands::Keypair {}) => {
                create_keypair().await?;
                info!("keypair successfully created")
            }
            Some(Commands::Sign { artifact }) => {
                let name = artifact.split("/").last().unwrap();
                let res = sign_artifact(name.to_string(), artifact.to_string()).await;
                if res.is_err() {
                    error!(
                        "{:#?}",
                        res.err().as_ref().unwrap().to_string().to_lowercase()
                    );
                }
                info!("artifact {} successfully signed", name);
            }
            Some(Commands::Verify { artifact }) => {
                let name = artifact.split("/").last().unwrap();
                let res = verify_artifact(name.to_string(), artifact.to_string()).await;
                if res.is_err() {
                    error!(
                        "{:#?}",
                        res.as_ref().err().unwrap().to_string().to_lowercase()
                    );
                }
                match res.as_ref().unwrap() {
                    true => info!("artifact {} is trusted", name),
                    false => warn!("artitact {} is not trusted", name),
                }
            }
            Some(Commands::Start {
                node,
                service,
                config_file,
                working_dir,
            }) => {
                let api_params = APIParameters {
                    command: "start".to_string(),
                    node: node.clone(),
                    service: service.clone(),
                    config_file: Some(config_file.clone()),
                    working_dir: Some(working_dir.clone()),
                    from_registry: Some(false),
                    skip_tls_verify: Some(true),
                    ip: None,
                    subnet: None,
                };
                let message = serde_json::to_string(&api_params).unwrap();
                let res = send_message(message, server_ip).await;
                if res.is_err() {
                    error!(
                        "send message {}",
                        res.err().unwrap().to_string().to_lowercase()
                    );
                } else {
                    info!("start message sent");
                }
            }
            Some(Commands::Stop {
                node,
                service,
                config_file,
                working_dir,
            }) => {
                let api_params = APIParameters {
                    command: "stop".to_string(),
                    node: node.clone(),
                    service: service.clone(),
                    config_file: Some(config_file.clone()),
                    working_dir: Some(working_dir.clone()),
                    from_registry: Some(false),
                    skip_tls_verify: Some(true),
                    ip: None,
                    subnet: None,
                };
                let message = serde_json::to_string(&api_params).unwrap();
                let res = send_message(message, server_ip).await;
                if res.is_err() {
                    error!(
                        "send message {}",
                        res.err().unwrap().to_string().to_lowercase()
                    );
                } else {
                    info!("stop message sent");
                }
            }
            Some(Commands::List {}) => {
                let api_params = APIParameters {
                    command: "list".to_string(),
                    node: "all".to_string(),
                    service: "".to_string(),
                    config_file: Some("".to_string()),
                    working_dir: Some("".to_string()),
                    from_registry: Some(true),
                    skip_tls_verify: Some(true),
                    ip: None,
                    subnet: None,
                };
                let message = serde_json::to_string(&api_params).unwrap();
                let res = send_message(message, server_ip).await;
                if res.is_err() {
                    error!(
                        "send message {}",
                        res.err().unwrap().to_string().to_lowercase()
                    );
                } else {
                    info!("list message sent");
                }
            }
            Some(Commands::RemoteExecute { node }) => {
                remote_execute(node.clone());
            }
            Some(Commands::RemoteUpload { node, file }) => {
                remote_upload(node.clone(), file.clone()).await;
            }
            Some(Commands::CreateBridge {
                node,
                name,
                ip,
                subnet,
            }) => {
                let api_params = APIParameters {
                    command: "create_bridge".to_string(),
                    node: node.to_string(),
                    service: name.to_string(),
                    config_file: None,
                    working_dir: None,
                    from_registry: None,
                    skip_tls_verify: None,
                    ip: Some(ip.to_string()),
                    subnet: Some(*subnet),
                };
                let message = serde_json::to_string(&api_params).unwrap();
                let res = send_message(message, server_ip).await;
                if res.is_err() {
                    error!(
                        "send message {}",
                        res.err().unwrap().to_string().to_lowercase()
                    );
                } else {
                    info!("list message sent");
                }
            }

            None => {
                error!("sub command not recognized, use --help to get list of cli options");
                process::exit(1);
            }
        },
    }
    Ok(())
}
