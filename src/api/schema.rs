use clap::{Parser, Subcommand};
use serde_derive::{Deserialize, Serialize};

/// rust-microservice-package-manager cli struct
#[derive(Parser)]
#[command(name = "rust-microservice-package-manager")]
#[command(author = "Luigi Mario Zuccarelli <luzuccar@redhat.com>")]
#[command(version = "0.0.1")]
#[command(about = "A simple command line tool that packages and signs microservice binaries in oci format. It als has an inbuilt runtime engine to launch & mangage microservices", long_about = None)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
    /// set the loglevel
    #[arg(
        value_enum,
        short,
        long,
        value_name = "loglevel",
        default_value = "info",
        help = "Set the log level [possible values: info, debug, trace]"
    )]
    pub loglevel: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Package subcommand (package a signed binary in oci image format)
    Package {
        #[arg(
            short,
            long,
            value_name = "config-file",
            help = "The config file used to package and sign microservice binaries (required)"
        )]
        config_file: String,
        #[arg(
            short,
            long,
            value_name = "working-dir",
            help = "The base working directory to store generated artifacts (required)"
        )]
        working_dir: String,
    },
    /// Stage subcommand (used to pull oci images from a registry and verify binaries are signed)
    Stage {
        #[arg(
            short,
            long,
            value_name = "config-file",
            help = "The config file used to package and sign microservice binaries (required)"
        )]
        config_file: String,
        #[arg(
            short,
            long,
            value_name = "working-dir",
            help = "The base working directory to hold the untarred binary files (required)"
        )]
        working_dir: String,
    },

    /// CreateReferralManifest subcommand (to build signed artifact manifests)
    CreateReferralManifest {
        #[arg(short, long, value_name = "name", help = "Component name (required)")]
        name: String,
        #[arg(
            short,
            long,
            value_name = "referral-url-digest",
            help = "The referral url with digest (required)"
        )]
        referral_url_digest: String,
        #[arg(
            short,
            long,
            value_name = "referral-size",
            help = "The referral manifest size (required)"
        )]
        referral_size: i64,
        #[arg(
            short,
            long,
            value_enum,
            help = "The image format oci or dockerv2 (required)"
        )]
        format: String,
    },

    /// Keypair (create PEM keypair)
    Keypair {},
    /// Sign
    Sign {
        #[arg(
            short,
            long,
            value_name = "artifact",
            help = "The artfifact to sign (required)"
        )]
        artifact: String,
    },
    /// Verify
    Verify {
        #[arg(
            short,
            long,
            value_name = "artifact",
            help = "The artfifact to verify (required)"
        )]
        artifact: String,
    },
}

#[derive(Serialize, Deserialize)]
pub struct BaseConfig {
    #[serde(rename = "created")]
    created: String,

    #[serde(rename = "architecture")]
    architecture: String,

    #[serde(rename = "os")]
    os: String,

    #[serde(rename = "config")]
    config: Config,

    #[serde(rename = "rootfs")]
    rootfs: Rootfs,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    #[serde(rename = "Env")]
    env: Vec<String>,

    #[serde(rename = "Cmd")]
    cmd: Vec<String>,

    #[serde(rename = "Labels")]
    labels: Labels,
}

#[derive(Serialize, Deserialize)]
pub struct Labels {
    #[serde(rename = "architecture")]
    architecture: String,

    #[serde(rename = "build-date")]
    build_date: String,

    #[serde(rename = "component")]
    component: String,

    #[serde(rename = "description")]
    description: String,

    #[serde(rename = "distribution-scope")]
    distribution_scope: String,

    #[serde(rename = "maintainer")]
    maintainer: String,

    #[serde(rename = "name")]
    name: String,

    #[serde(rename = "release")]
    release: String,

    #[serde(rename = "summary")]
    summary: String,

    #[serde(rename = "url")]
    url: String,

    #[serde(rename = "vcs-ref")]
    vcs_ref: String,

    #[serde(rename = "vcs-type")]
    vcs_type: String,

    #[serde(rename = "vendor")]
    vendor: String,

    #[serde(rename = "version")]
    version: String,
}

#[derive(Serialize, Deserialize)]
pub struct Rootfs {
    #[serde(rename = "type")]
    rootfs_type: String,

    #[serde(rename = "diff_ids")]
    diff_ids: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OCIIndex {
    pub schema_version: i64,
    pub manifests: Vec<Layer>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    pub media_type: String,
    pub size: i64,
    pub digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Annotations>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Manifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "digest")]
    pub digest: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "mediaType")]
    pub media_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "artifactType")]
    pub artifact_type: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "platform")]
    pub platform: Option<ManifestPlatform>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "size")]
    pub size: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "config")]
    pub config: Option<Layer>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "layers")]
    pub layers: Option<Vec<Layer>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "subject")]
    pub subject: Option<Layer>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ManifestPlatform {
    #[serde(rename = "architecture")]
    pub architecture: String,

    #[serde(rename = "os")]
    pub os: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Annotations {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "org.opencontainers.image.title")]
    pub image_title: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "org.opencontainers.image.created")]
    pub image_created: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SignatureJson {
    #[serde(rename = "artitact")]
    pub artifact: String,
    #[serde(rename = "signature")]
    pub signature: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MicroserviceConfig {
    #[serde(rename = "apiVersion")]
    api_version: String,

    #[serde(rename = "kind")]
    kind: String,

    #[serde(rename = "spec")]
    pub spec: Spec,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Spec {
    #[serde(rename = "services")]
    pub services: Vec<Service>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Service {
    /// name must correspond to the actual binary that was created
    #[serde(rename = "name")]
    pub name: String,

    /// binary_path is the path to the actual microservice project on disk
    /// and the link to the binary
    #[serde(rename = "binaryPath")]
    pub binary_path: String,

    /// registry is the oci registry to pull the image from
    #[serde(rename = "registry")]
    pub registry: String,

    #[serde(rename = "version")]
    pub version: String,

    #[serde(rename = "authors")]
    pub authors: Vec<String>,

    #[serde(rename = "description")]
    pub description: String,

    #[serde(rename = "env")]
    pub env: Option<Vec<KeyValue>>,

    #[serde(rename = "args")]
    pub args: Option<Vec<KeyValue>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct KeyValue {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "value")]
    pub value: String,
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub enum Imageformat {
    OCI,
    DOCKERV2,
}
