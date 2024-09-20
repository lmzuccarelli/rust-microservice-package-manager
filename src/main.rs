use crate::api::schema::*;
use crate::config::read::*;
use crate::package::build::*;
use clap::Parser;
use custom_logger::*;
use mirror_error::MirrorError;
use mirror_utils::fs_handler;
use package::signing::{create_keypair, sign_artifact, verify_artifact};
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
            log.info(&format!("working-dir {}", working_dir));
            log.info(&format!("Microservices struct {:#?}", sc));
            for service in sc.spec.services.iter() {
                create_tar_gz(service.name.clone(), service.project.clone()).await?;
            }
        }
        Some(Commands::Keypair {}) => {
            create_keypair().await?;
        }
        Some(Commands::Sign { artifact }) => {
            sign_artifact(artifact.to_string()).await?;
        }
        Some(Commands::Verify { artifact }) => {
            verify_artifact(artifact.to_string()).await?;
        }
        None => {
            log.error("sub command not recognized");
            process::exit(1);
        }
    }
    Ok(())
}
