//! Visual and terminal satellites domain.

pub mod ai;
pub mod img;
pub mod pick;

pub fn dispatch(cmd: &str, args: &[String]) -> bool {
    match cmd {
        "img" | "image" | "view" => {
            img::run(args);
            true
        }
        "pick" | "hints" => {
            pick::run(args);
            true
        }
        "ai" | "ask" | "fix" => {
            let mut all_args = Vec::new();
            if cmd != "ai" {
                all_args.push(cmd.to_string());
            }
            all_args.extend_from_slice(args);
            ai::run(&all_args);
            true
        }
        _ => false,
    }
}
