use crate::api::schema::Service;
use custom_logger::*;
use mirror_error::MirrorError;
use std::process::{Command, Stdio};

pub async fn start_service(
    log: &Logging,
    working_dir: String,
    service: Service,
) -> Result<(), MirrorError> {
    let binary = format!(
        "{}/microservices/{}/{}",
        working_dir, service.name, service.name
    );
    let mut output = Command::new(binary);
    if service.args.is_some() {
        for arg in service.args.unwrap().iter() {
            output.arg(arg.name.clone());
            output.arg(arg.value.clone());
        }
    }
    let res = output.output();
    if res.is_err() {
        return Err(MirrorError::new(&format!(
            "{}",
            String::from_utf8_lossy(&res.unwrap().stderr)
        )));
    }
    if res.is_ok() {
        let response = format!("{}", String::from_utf8_lossy(&res.as_ref().unwrap().stdout));
        let err_response = format!("{}", String::from_utf8_lossy(&res.as_ref().unwrap().stderr));
        if err_response.contains("ERROR") {
            return Err(MirrorError::new(&format!(
                "{}",
                String::from_utf8_lossy(&res.unwrap().stderr)
            )));
        }
        log.info(&response);
    }
    Ok(())
}

pub async fn stop_service(log: &Logging, service: String) -> Result<(), MirrorError> {
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
    log.info(&format!("[stop_service] {}", result));

    Ok(())
}
