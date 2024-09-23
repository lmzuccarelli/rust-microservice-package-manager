use mirror_error::MirrorError;
use mirror_utils::fs_handler;
use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::rsa::Rsa;
use openssl::sign::{Signer, Verifier};
use std::fs::File;
use std::io::Read;
use std::io::Write;

// Generate a keypair
pub async fn create_keypair() -> Result<(), MirrorError> {
    fs_handler(".ssh".to_string(), "create_dir", None).await?;
    let private = Rsa::generate(2048).unwrap();
    let public = PKey::from_rsa(private.clone()).unwrap();
    let mut f_prv = File::create(".ssh/private.pem").expect("should create file");
    let res_prv = f_prv.write_all(&private.private_key_to_pem().unwrap());
    let metadata = f_prv.metadata().unwrap();
    let mut permissions = metadata.permissions();
    permissions.set_readonly(true);
    if res_prv.is_err() {
        let err = MirrorError::new(&format!(
            "writing private key {}",
            res_prv.err().unwrap().to_string().to_lowercase()
        ));
        return Err(err);
    }
    let mut f_pub = File::create(".ssh/public.pem").expect("should create file");
    let metadata = f_pub.metadata().unwrap();
    let mut permissions = metadata.permissions();
    permissions.set_readonly(true);
    let res_pub = f_pub.write_all(&public.public_key_to_pem().unwrap());
    if res_pub.is_err() {
        let err = MirrorError::new(&format!(
            "writing blob {}",
            res_pub.err().unwrap().to_string().to_lowercase()
        ));
        return Err(err);
    }
    Ok(())
}

pub async fn sign_artifact(name: String, file: String) -> Result<(), MirrorError> {
    let mut artifact_buf = vec![];
    let res_file = File::open(file.clone());
    if res_file.is_err() {
        let err = MirrorError::new(&format!(
            "opening artifact file {}",
            res_file.err().unwrap().to_string().to_lowercase()
        ));
        return Err(err);
    }
    let res_r = res_file.unwrap().read_to_end(&mut artifact_buf);
    if res_r.is_err() {
        let err = MirrorError::new(&format!(
            "reading artifact file {}",
            res_r.err().unwrap().to_string().to_lowercase()
        ));
        return Err(err);
    }
    let res_prv = File::open(".ssh/private.pem");
    if res_prv.is_err() {
        let err = MirrorError::new(&format!(
            "opening private key {}",
            res_r.err().unwrap().to_string().to_lowercase()
        ));
        return Err(err);
    }
    let mut buf = vec![];
    let res_r = res_prv.unwrap().read_to_end(&mut buf);
    if res_r.is_err() {
        let err = MirrorError::new(&format!(
            "reading private key {}",
            res_r.err().unwrap().to_string().to_lowercase()
        ));
        return Err(err);
    }
    let public_key = PKey::private_key_from_pem(&buf);

    // Sign the data
    let mut signer = Signer::new(MessageDigest::sha256(), &public_key.unwrap()).unwrap();
    signer.update(&artifact_buf).unwrap();
    let mut f_sign = File::create(file.clone() + &".signed").expect("should create signed file");
    let res_sign = f_sign.write_all(&artifact_buf);
    if res_sign.is_err() {
        let err = MirrorError::new(&format!(
            "writing signed artifact {}",
            res_sign.err().unwrap().to_string().to_lowercase()
        ));
        return Err(err);
    }
    let signature = signer.sign_to_vec().unwrap();
    let res_signature = File::create(format!(".ssh/{}-signature", name));
    if res_signature.is_err() {
        let err = MirrorError::new(&format!(
            "creating signature {}",
            res_signature.err().unwrap().to_string().to_lowercase()
        ));
        return Err(err);
    }
    let res_sig = res_signature.unwrap().write_all(&signature);
    if res_sig.is_err() {
        let err = MirrorError::new(&format!(
            "writing signature artifact {}",
            res_sig.err().unwrap().to_string().to_lowercase()
        ));
        return Err(err);
    }
    Ok(())
}

pub async fn verify_artifact(name: String, file: String) -> Result<bool, MirrorError> {
    // Verify the data
    let mut tar_buf = vec![];
    let mut tgz_file = File::open(file.clone()).unwrap();
    let res_r = tgz_file.read_to_end(&mut tar_buf);
    if res_r.is_err() {
        let err = MirrorError::new(&format!(
            "signing file {}",
            res_r.err().unwrap().to_string().to_lowercase()
        ));
        return Err(err);
    }

    let mut f_prv = File::open(".ssh/public.pem").unwrap();
    let mut buf = vec![];
    f_prv.read_to_end(&mut buf).expect("should read public key");
    let public_key = PKey::public_key_from_pem(&buf);
    let res_sig = File::open(format!(".ssh/{}-signature", name));
    if res_sig.is_err() {
        return Ok(false);
    }
    let mut signature_buf = vec![];
    let res = res_sig.unwrap().read_to_end(&mut signature_buf);
    if res.is_err() {
        let err = MirrorError::new(&format!(
            "reading to buffer {}",
            res.err().unwrap().to_string().to_lowercase()
        ));
        return Err(err);
    }
    let mut verifier =
        Verifier::new(MessageDigest::sha256(), &public_key.as_ref().unwrap()).unwrap();
    verifier.update(&tar_buf).unwrap();
    let res = verifier.verify(&signature_buf).unwrap();
    Ok(res)
}
