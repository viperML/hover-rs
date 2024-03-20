mod utils;

use crate::utils::NNONE;

use eyre::Context;
use nix::mount::{mount, MsFlags};
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{getgid, getpid, getuid};
use std::fs::OpenOptions;
use std::io::Write;
use std::time::SystemTime;
use std::{env, fs};
use std::{os::unix::process::CommandExt, path::PathBuf};
use tracing::{debug, error, warn};

#[derive(Debug, clap::Parser)]
struct Args {
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

    if let None = env::var("HOVER").ok() {
        // TODO: allow stacked hovers, keep track of hover level
        error!("You are hovering too much!");
    };

    let app_cache = env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| env::var("HOME").map(PathBuf::from).unwrap().join(".cache"))
        .join("hover-rs");
    std::fs::create_dir_all(&app_cache)?;

    let allocation = {
        let format = time::format_description::parse("[year]-[month]-[day]-[hour][minute]")?;
        let now: time::OffsetDateTime = SystemTime::now().into();
        let mytime = now.format(&format)?;
        use rand::Rng;
        let seed: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(7)
            .map(char::from)
            .collect();
        format!("{mytime}-{seed}")
    };

    let app_runtime = env::var("XDG_RUNTIME_DIR")
        .map(|s| PathBuf::from(s).join("hover-rs"))
        .unwrap_or_else(|_| PathBuf::from("/tmp").join(format!("hover-rs-{allocation}")));
    std::fs::create_dir_all(&app_runtime)?;

    let target = PathBuf::from(env::var("HOME")?);

    let uid = getuid().as_raw();
    let gid = getgid().as_raw();
    let pid = getpid().as_raw();
    debug!(?uid, ?gid, ?pid);

    unshare(CloneFlags::CLONE_NEWUSER | CloneFlags::CLONE_NEWNS)?;

    {
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/proc/self/uid_map"))?;
        let msg = format!("0 {uid} 1");
        write!(&mut f, "{msg}")?;
    }

    {
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/proc/self/setgroups"))?;
        write!(&mut f, "deny")?;
    }

    {
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/proc/self/gid_map"))?;
        let msg = format!("0 {gid} 1");
        write!(&mut f, "{msg}")?;
    }

    mount(
        Some("tmpfs"),
        &app_runtime,
        Some("tmpfs"),
        MsFlags::empty(),
        NNONE,
    )?;

    let ro_mount = app_runtime.join("oldroot");
    fs::create_dir_all(&ro_mount)?;

    // Mount root dir as RO
    mount(Some(&target), &ro_mount, NNONE, MsFlags::MS_BIND, NNONE)?;

    mount(
        NNONE,
        &ro_mount,
        NNONE,
        MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY,
        NNONE,
    )?;

    let newroot = app_runtime.join("newroot");
    fs::create_dir_all(&newroot)?;

    let layer_dir = app_cache.join(format!("layer-{allocation}"));
    fs::create_dir_all(&layer_dir)?;
    let work_dir = app_cache.join(format!(".work-{allocation}"));
    fs::create_dir_all(&work_dir)?;

    mount(
        Some("overlay"),
        &newroot,
        Some("overlay"),
        MsFlags::empty(),
        Some(
            format!(
                "lowerdir={},upperdir={},workdir={}",
                ro_mount.to_string_lossy(),
                layer_dir.to_string_lossy(),
                work_dir.to_string_lossy()
            )
            .as_str(),
        ),
    )?;

    mount(Some(&newroot), &target, NNONE, MsFlags::MS_BIND, NNONE)?;

    {
        use owo_colors::OwoColorize;
        println!("You are now {}", "hovering~".blink());
        println!(
            "  A layer is covering your {}",
            target.to_string_lossy().bold().red()
        );
        println!(
            "  You can find the layer leftovers at: {}",
            layer_dir.to_string_lossy().bold().red()
        );
    }

    env::set_var("HOVER", "1");

    let mut command = args.command.into_iter();
    let argv0 = command
        .next()
        .or_else(|| env::var("SHELL").ok())
        .unwrap_or(String::from("sh"));

    Err(std::process::Command::new(argv0).args(command).exec())
        .wrap_err("Running the provided command")

    // Ok(())
}
