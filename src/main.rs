mod utils;

use crate::utils::{callback_wrapper, NNONE};

use caps::{CapSet, Capability};
use eyre::{bail, ensure, Context};
use nix::libc::{gid_t, uid_t, SIGCHLD, SIGKILL};
use nix::mount::{mount, MsFlags};
use nix::sched::{clone, setns, unshare, CloneFlags};
use nix::sys::prctl::set_pdeathsig;
use nix::sys::signal::Signal;
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::{fork, getgid, getpid, getuid, Gid, Pid, Uid, User};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::fd::AsFd;
use std::path::Path;
use std::process::{exit, Command};
use std::time::{Duration, SystemTime};
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

    if let Some(_) = env::var("HOVER").ok() {
        // TODO: allow stacked hovers, keep track of hover level
        bail!("You are hovering too much!");
    };

    let uid = getuid();
    let gid = getgid();
    let pid = getpid();
    debug!(?uid, ?gid, ?pid);

    ensure!(!uid.is_root(), "hover-rs is not made to be run as root!");

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

    let (argv0, argv) = if args.command.is_empty() {
        (env::var("SHELL").ok().unwrap_or(String::from("sh")), vec![])
    } else {
        let mut _args = args.command.into_iter();
        (_args.next().unwrap(), _args.collect())
    };
    let mut cmd = Command::new(argv0);
    cmd.args(argv);

    let mut stack = [0; 4000];
    let child = unsafe {
        clone(
            Box::new(move || {
                set_pdeathsig(Some(Signal::SIGTERM)).unwrap();
                slave(&app_runtime, &app_cache, &allocation, uid, gid).unwrap();
                // Command::new(env::var("SHELL")?).exec();
                cmd.exec();
                todo!()
            }),
            &mut stack,
            CloneFlags::CLONE_NEWUSER | CloneFlags::CLONE_NEWNS,
            Some(SIGCHLD),
        )
    }?;

    let privileged = {
        let mycaps = caps::read(None, CapSet::Effective)?;
        mycaps.contains(&Capability::CAP_SETUID) && mycaps.contains(&Capability::CAP_SETGID)
    };

    debug!(?privileged);

    {
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/proc/{child}/uid_map"))?;
        let msg = format!("0 {uid} 1");
        f.write(msg.as_bytes())
            .wrap_err("Setting uid_map for child process")?;
    }
    {
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/proc/{child}/setgroups"))?;
        f.write("deny".as_bytes())
            .wrap_err("Setting setgroups for child process")?;
    }
    {
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/proc/{child}/gid_map"))?;
        let msg = format!("0 {gid} 1");
        f.write(msg.as_bytes())
            .wrap_err("Setting gid_map for child process")?;
    }

    let ret = waitpid(child, None)?;
    if let WaitStatus::Exited(_, 0) = ret {
        debug!(?ret);
    } else {
        error!(?ret);
    }

    Ok(())
}

fn slave(
    app_runtime: &Path,
    app_cache: &Path,
    allocation: &str,
    uid: Uid,
    gid: Gid,
) -> eyre::Result<()> {
    let target = PathBuf::from(env::var("HOME")?);

    // TODO sync
    std::thread::sleep(Duration::from_millis(200));

    mount(
        Some("tmpfs"),
        app_runtime,
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

    unshare(CloneFlags::CLONE_NEWUSER)?;

    {
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/proc/self/uid_map")?;
        let msg = format!("{uid} 0 1");
        f.write(msg.as_bytes())?;
    }
    {
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/proc/self/gid_map")?;
        let msg = format!("{gid} 0 1");
        f.write(msg.as_bytes())?;
    }

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

    Ok(())
}
