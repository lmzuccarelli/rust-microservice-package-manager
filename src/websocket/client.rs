use crate::workflow::handler;
use crate::{api::schema::APIParameters, APIResponse};
use custom_logger::*;
use futures_util::stream::StreamExt;
use futures_util::SinkExt;
use gethostname::gethostname;
use http::Uri;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_websockets::{ClientBuilder, Message};

pub async fn start_client(log: &Logging) -> Result<(), tokio_websockets::Error> {
    let (mut ws_stream, _) = ClientBuilder::from_uri(Uri::from_static("ws://127.0.0.1:2000"))
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
                                log.info(&format!("response {:?}",api_response_result.unwrap()));
                            } else {
                                // if its not ok try the APIParameters
                                let api_params: APIParameters = serde_json::from_str(&json_data).unwrap();
                                let mut message =  APIResponse {
                                    status: "KO".to_string(),
                                    text: "ko".to_string(),
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
                                        } else {
                                            message.status = "KO".to_string();
                                            message.text = format!("hostname {} does not match node {} parameter",
                                                gethostname().to_string_lossy().to_string(),
                                                api_params.node);
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

pub async fn send_message(log: &Logging, message: String) -> Result<(), tokio_websockets::Error> {
    let (mut ws_stream, _) = ClientBuilder::from_uri(Uri::from_static("ws://127.0.0.1:2000"))
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
