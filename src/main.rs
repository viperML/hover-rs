mod utils;

use crate::utils::NNONE;

use eyre::{Context, ContextCompat};
use nix::mount::{mount, MsFlags};
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{getgid, getpid, getuid};
use std::fs::OpenOptions;
use std::io::Write;
use std::{env, fs};
use std::{os::unix::process::CommandExt, path::PathBuf};
use tracing::debug;

#[derive(Debug, clap::Parser)]
struct Args {
    #[arg(short, long)]
    tmp_path: Option<PathBuf>,

    // #[arg(last = true)]
    #[arg(trailing_var_arg = true)]
    command: Vec<String>,
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    {
        use tracing_subscriber::{fmt, prelude::*, EnvFilter};
        tracing_subscriber::registry()
            .with(fmt::layer().without_time().with_line_number(true))
            .with(EnvFilter::from_default_env())
            .init();
    }

    let args = <Args as clap::Parser>::parse();
    debug!(?args);

    let xdg = microxdg::XdgApp::new("hover-rs")?;
    let xdg_cache = xdg.app_cache()?;
    let xdg_runtime = xdg.runtime()?.wrap_err("XDG_RUNTIME_DIR not set")?;

    std::fs::create_dir_all(&xdg_cache)?;
    std::fs::create_dir_all(&xdg_runtime)?;

    let uid = getuid().as_raw();
    let gid = getgid().as_raw();
    let pid = getpid().as_raw();
    debug!(?uid, ?gid, ?pid);

    unshare(CloneFlags::CLONE_NEWUSER | CloneFlags::CLONE_NEWNS)?;

    {
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/proc/{pid}/uid_map"))?;
        let msg = format!("0 {uid} 1");
        write!(&mut f, "{msg}")?;
    }

    {
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/proc/{pid}/setgroups"))?;
        write!(&mut f, "deny")?;
    }

    {
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/proc/{pid}/gid_map"))?;
        let msg = format!("0 {gid} 1");
        write!(&mut f, "{msg}")?;
    }

    let oldroot = xdg_runtime.join("oldroot");
    fs::create_dir_all(&oldroot)?;

    // Mount root dir as RO
    mount(Some(xdg.home()), &oldroot, NNONE, MsFlags::MS_BIND, NNONE)?;

    mount(
        NNONE,
        &oldroot,
        NNONE,
        MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY,
        NNONE,
    )?;

    let newroot = xdg_runtime.join("newroot");
    fs::create_dir_all(&newroot)?;

    let layer = xdg_cache.join("layer");
    fs::create_dir_all(&layer)?;
    let work = xdg_cache.join("work");
    fs::create_dir_all(&work)?;

    mount(
        Some("overlay"),
        &newroot,
        Some("overlay"),
        MsFlags::empty(),
        Some(
            format!(
                "lowerdir={},upperdir={},workdir={}",
                oldroot.to_string_lossy(),
                layer.to_string_lossy(),
                work.to_string_lossy()
            )
            .as_str(),
        ),
    )?;

    let mut command = args.command.into_iter();
    let argv0 = command
        .next()
        .or_else(|| env::var("SHELL").ok())
        .unwrap_or(String::from("sh"));

    Err(std::process::Command::new(argv0).args(command).exec())
        .wrap_err("Running the provided command")

    // Ok(())
}
