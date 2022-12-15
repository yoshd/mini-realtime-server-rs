use std::fmt::Debug;

#[derive(Clone, Debug)]
pub struct Config {
    pub auth: Auth,
    pub tls: Tls,
}

#[derive(Clone, Debug)]
pub struct Auth {
    pub enable_bearer: bool,
    pub bearer: String,
}

#[derive(Clone, Debug)]
pub struct Tls {
    pub enable: bool,
    pub cert_file_path: String,
    pub key_file_path: String,
}
