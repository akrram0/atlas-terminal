//! Developer productivity tools domain.

pub mod diff;
pub mod json;
pub mod jwt;
pub mod ports;
pub mod qr;

pub fn dispatch(cmd: &str, args: &[String]) -> bool {
    match cmd {
        "port" | "ports" => {
            ports::run(args);
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
