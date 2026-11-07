//! System and core tools domain.

pub mod bench;
pub mod config;
pub mod env;
pub mod hud;
pub mod theme;
pub mod version;

pub fn dispatch(cmd: &str, args: &[String]) -> bool {
    match cmd {
        "version" | "-v" | "--version" => {
            version::run();
            true
        }
        "info" | "env" => {
            env::run();
            true
        }
        "theme" => {
            theme::run(args);
            true
        }
        "config" => {
            config::run(args);
            true
        }
        "hud" | "island" => {
            hud::run(args);
            true
        }
        "bench" | "test-render" => {
            bench::run();
            true
        }
        _ => false,
    }
}
