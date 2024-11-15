use std::str::FromStr;

use crate::workflow::handler;
use crate::{api::schema::APIParameters, APIResponse};
use custom_logger::*;
use futures_util::stream::StreamExt;
use futures_util::SinkExt;
use gethostname::gethostname;
use http::Uri;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_websockets::{ClientBuilder, Message};

pub async fn start_client(log: &Logging, server_ip: String) -> Result<(), tokio_websockets::Error> {
    let address = &format!("ws://{}:2000", server_ip);
    let (mut ws_stream, _) = ClientBuilder::from_uri(Uri::from_str(address).unwrap())
        .connect()
        .await?;

    let stdin = tokio::io::stdin();
    let mut stdin = BufReader::new(stdin).lines();

    // Continuous loop for concurrently sending and receiving messages.
    loop {
        tokio::select! {
            incoming = ws_stream.next() => {
                match incoming {
                    Some(Ok(msg)) => {
                        let text = msg.as_text();
                        if text.is_some() {
                            let json_data = text.unwrap();
                            // check if its a response
                            let api_response_result = serde_json::from_str::<APIResponse>(&json_data);
                            if api_response_result.is_ok() {
                                let res  = api_response_result.unwrap();
                                if res.status == "KO" {
                                    log.error(&format!("{}",res.text));
                                } else {
                                    log.info(&format!("{}",res.text));
                                }
                            } else {
                                // if its not ok try the APIParameters
                                let api_params: APIParameters = serde_json::from_str(&json_data).unwrap();
                                let mut message =  APIResponse {
                                    status: "".to_string(),
                                    text: "".to_string(),
                                    node: "".to_string(),
                                    service: "".to_string(),
                                };
                                match api_params.command.as_str() {
                                    "package" => {
                                        if api_params.node == gethostname().to_string_lossy().to_string() {
                                            let res = handler::package(
                                                log,
                                                api_params.working_dir,
                                                api_params.config_file,
                                                api_params.skip_tls_verify,
                                            )
                                            .await;
                                            if res.is_err() {
                                                message.status = "KO".to_string();
                                                message.text = format!("package error {}",res.err().unwrap().to_string().to_lowercase());
                                            } else {
                                                message.text = format!("package completed successfully");
                                            }
                                        }
                                    },
                                    "stage" => {
                                        let res = handler::stage(
                                            log,
                                            api_params.from_registry,
                                            api_params.working_dir,
                                            api_params.config_file,
                                            api_params.skip_tls_verify,
                                        )
                                        .await;
                                        if res.is_err() {
                                            message.status = "KO".to_string();
                                            message.text = format!("staging error {}",res.err().unwrap().to_string().to_lowercase());
                                        } else {
                                           message.status = "OK".to_string();
                                           message.text = format!("staging completed successfully");
                                        }
                                    },
                                    "list" => {
                                        let res = handler::list(
                                            log,
                                        )
                                        .await;
                                        if res.is_err() {
                                            message.status = "KO".to_string();
                                            message.text = format!("list error {}",res.err().unwrap().to_string().to_lowercase());
                                        } else {
                                            message.status = "OK".to_string();
                                            message.service = "list".to_string();
                                            message.node = gethostname().to_string_lossy().to_string();
                                            message.text = res.unwrap();
                                        }
                                    },
                                    "start" => {
                                        let res = handler::start(
                                            log,
                                            api_params.service.clone(),
                                            api_params.working_dir,
                                            api_params.config_file,
                                        )
                                        .await;
                                        if res.is_err() {
                                            message.status = "KO".to_string();
                                            message.text = format!("{}",res.err().unwrap().to_string().to_lowercase());
                                        } else {
                                            message.status = "OK".to_string();
                                            message.service = api_params.service.to_string();
                                            message.node = gethostname().to_string_lossy().to_string();
                                            message.text = "started".to_string();
                                        }
                                    },
                                    "stop" => {
                                        let res = handler::stop(
                                            log,
                                            api_params.service.clone(),
                                        )
                                        .await;
                                        if res.is_err() {
                                            message.status = "KO".to_string();
                                            message.text = format!("stop service error {}",res.err().unwrap().to_string().to_lowercase());
                                        } else {
                                            message.status = "OK".to_string();
                                            message.service = api_params.service.to_string();
                                            message.node = gethostname().to_string_lossy().to_string();
                                            message.text = "stopped".to_string();
                                        }
                                    },
                                    &_ => {
                                        message.status = "KO".to_string();
                                        message.text = format!("incorrect command (not supported) {}",api_params.command);
                                    },
                                }
                                let json_response = serde_json::to_string(&message).unwrap();
                                ws_stream.send(Message::text(json_response)).await?;
                            }
                        }
                    },
                    Some(Err(err)) => return Err(err.into()),
                    None => return Ok(()),
                }
            }
            res = stdin.next_line() => {
                match res {
                    Ok(None) => return Ok(()),
                    Ok(Some(line)) => ws_stream.send(Message::text(line.to_string())).await?,
                    Err(err) => return Err(err.into()),
                }
            }
        }
    }
}

pub async fn send_message(
    log: &Logging,
    message: String,
    server_ip: String,
) -> Result<(), tokio_websockets::Error> {
    let address = &format!("ws://{}:2000", server_ip);
    let (mut ws_stream, _) = ClientBuilder::from_uri(Uri::from_str(address).unwrap())
        .connect()
        .await?;

    ws_stream.send(Message::text(message)).await?;
    tokio::select! {
        incoming = ws_stream.next() => {
            match incoming {
                Some(Ok(msg)) => {
                    if let Some(text) = msg.as_text() {
                        log.debug(&format!("from server: {}", text));
                    }
                }
                Some(Err(err)) => return Err(err.into()),
                None => return Ok(()),
            }
        }
    }
    ws_stream.close().await?;
    Ok(())
}
