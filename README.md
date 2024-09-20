# A rust cli to package, sign verify and launch microservices 

A simple command line utility that takes a yaml config , compiles and builds microservices, signs and verifies the binaries (all in oci format) and has 
a built in runtime engine to manage (start, stop,  kill, restart) microservices

## Setup

Clone this repo 

```
git clone https://github.com/lmzuccarelli/rust-microservice-package-manager.git

```

Create an executable

```
cd rust-microservice-package-manager

make clean-all

# creates a release (optimized) executable
# in the ./target/release directory
make build

```

## Usage

Create a config yaml (refer to the one in the config directory)


```
apiVersion: microservices.appliaction.io/v1alpha1
kind: MicroserviceConfig
spec:
  services:
    - name: convey
      project: /home/lzuccarelli/Projects/convey/target/release
      version: 0.1.1
      authors: 
        - lmzuccarelli luzuccar@redhat.com
        - mtroisi hello@marcotroisi.com
      description: "Simple demo of a microservice binary to signed package"
      env:
        - name: LOG_LEVEL
          value: TRACE
      args:
        - name: "--config"
          value: "lb-setup.toml"
```

Execute the cli to compile to create a RSA (PEM) keypair to sign artifacts

```
./target/release/microservice-package-manager keypair
```

## Notes


