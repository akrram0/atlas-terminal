//! Developer productivity tools domain.

pub mod clip;
pub mod diff;
pub mod hash;
pub mod json;
pub mod jwt;
pub mod ports;
pub mod qr;
pub mod serve;

pub fn dispatch(cmd: &str, args: &[String]) -> bool {
    match cmd {
        "port" | "ports" => {
            ports::run(args);
            true
        }
        "serve" | "http" => {
            serve::run(args);
            true
        }
        "hash" | "crypto" => {
            hash::run(args);
            true
        }
        "clip" | "copy" => {
            clip::run(args);
            true
        }
        "jwt" => {
            jwt::run(args);
            true
        }
        "qr" => {
            qr::run(args);
            true
        }
        "json" | "jq" => {
            json::run(args);
            true
        }
        "diff" => {
            diff::run(args);
            true
        }
        _ => false,
    }
}
