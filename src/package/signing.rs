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

pub async fn sign_artifact(file: String) -> Result<(), MirrorError> {
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

    let mut f_prv = File::open(".ssh/private.pem").unwrap();
    let mut buf = vec![];
    f_prv
        .read_to_end(&mut buf)
        .expect("should read private key");
    let public_key = PKey::private_key_from_pem(&buf);

    // Sign the data
    let mut signer = Signer::new(MessageDigest::sha256(), &public_key.unwrap()).unwrap();
    signer.update(&tar_buf).unwrap();
    let mut f_sign = File::create(file.clone() + &".signed").expect("should create signed file");
    let res_sign = f_sign.write_all(&tar_buf);
    if res_sign.is_err() {
        let err = MirrorError::new(&format!(
            "writing signed artifact {}",
            res_sign.err().unwrap().to_string().to_lowercase()
        ));
        return Err(err);
    }
    let signature = signer.sign_to_vec().unwrap();
    let mut f_signature = File::create(".ssh/signature").expect("should create signature file");
    let res_sig = f_signature.write_all(&signature);
    if res_sig.is_err() {
        let err = MirrorError::new(&format!(
            "writing signature artifact {}",
            res_sig.err().unwrap().to_string().to_lowercase()
        ));
        return Err(err);
    }
    Ok(())
}

pub async fn verify_artifact(file: String) -> Result<(), MirrorError> {
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
    let mut f_signature = File::open(".ssh/signature").unwrap();
    let mut signature_buf = vec![];
    f_signature
        .read_to_end(&mut signature_buf)
        .expect("should read signature");

    let mut verifier =
        Verifier::new(MessageDigest::sha256(), &public_key.as_ref().unwrap()).unwrap();
    verifier.update(&tar_buf).unwrap();
    let res = verifier.verify(&signature_buf).unwrap();
    if res {
        println!("The artifact {} is trusted", file);
    } else {
        println!("The artifact {} is not trusted", file);
    }
    Ok(())
}
