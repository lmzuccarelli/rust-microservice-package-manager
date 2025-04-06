use crate::api::schema::Service;
use custom_logger::*;
use mirror_error::MirrorError;
use std::env::set_current_dir;
use std::process::{Command, Stdio};

pub async fn start_service(working_dir: String, service: Service) -> Result<(), MirrorError> {
    let dir = format!("{}/microservices/{}", working_dir, service.name);
    let _cd = set_current_dir(&dir);
    let mut start_ms = Command::new(format!("./{}", service.name));
    if service.args.is_some() {
        for arg in service.args.unwrap().iter() {
            start_ms.arg(arg.name.clone());
            start_ms.arg(arg.value.clone());
        }
    }
    let res = start_ms.spawn().unwrap().wait_with_output();
    if res.is_err() {
        return Err(MirrorError::new(&format!(
            "{}",
            String::from_utf8_lossy(&res.unwrap().stderr)
        )));
    }
    if res.is_ok() {
        let err_response = format!("{}", String::from_utf8_lossy(&res.as_ref().unwrap().stderr));
        if err_response.contains("ERROR") {
            return Err(MirrorError::new(&format!(
                "{}",
                String::from_utf8_lossy(&res.unwrap().stderr)
            )));
        }
    }
    let response = format!("{}", String::from_utf8_lossy(&res.as_ref().unwrap().stdout));
    info!("start_service] microservice {}", &response);
    Ok(())
}

pub async fn stop_service(service: String) -> Result<(), MirrorError> {
    let ps = Command::new("ps")
        .arg("ef")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let grep_main = Command::new("grep")
        .arg(service)
        .stdin(Stdio::from(ps.stdout.unwrap()))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let grep_ignore = Command::new("grep")
        .arg("-v")
        .arg("grep")
        .stdin(Stdio::from(grep_main.stdout.unwrap()))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let output = grep_ignore.wait_with_output().unwrap();
    let result = String::from_utf8_lossy(&output.stdout);
    info!("[stop_service] {}", result);
    Ok(())
}
