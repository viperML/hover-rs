
/// None but for nix's types
pub const NNONE: Option<&str> = None;

pub fn callback_wrapper<F, T, E>(inner: F) -> isize
where
    F: FnOnce() -> Result<T, E>,
    T: std::process::Termination,
    E: std::fmt::Debug,
{
    use std::process::ExitCode;
    use std::process::Termination;

    let res = inner();
    match res {
        Ok(_) => {
            res.report();
            0
        }
        Err(_) => {
            res.report();
            1
        }
    }
}
