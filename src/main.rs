#![allow(unused_imports)]
mod utils;

use nix::mount::{mount, MsFlags};
use nix::unistd::{getgid, getpid, getuid};
use nix::{
    errno::Errno,
    libc::SIGCHLD,
    sched::{clone, unshare, CloneFlags},
    sys::{
        signal::Signal,
        wait::{wait, waitid, waitpid, WaitPidFlag},
    },
    unistd::{setuid, Pid, Uid},
};
use rand::{distributions::Alphanumeric, Rng};
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::{fs::File, os::unix::process::CommandExt, path::PathBuf};
use std::{io::Write, time::Duration};
use tracing::{debug, info, span, Level};

use crate::utils::{callback_wrapper, NNONE};

#[derive(Debug, clap::Parser)]
struct Args {
    tmp_path: Option<PathBuf>,
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

    let mut rng = rand::thread_rng();
    let unique: String = (0..6).map(|_| rng.sample(Alphanumeric) as char).collect();
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::parse("[year]-[month]-[day]")?)?;

    let tmp_path = PathBuf::from(std::env::var("XDG_CACHE_HOME")?)
        .join("hover-rs")
        .join(format!("{now}.{unique}"));
    debug!(?tmp_path);
    info!("Temp path: {}", tmp_path.to_string_lossy());

    // std::fs::create_dir_all(tmp_path)?;

    let pid = Pid::this();
    let span = span!(Level::DEBUG, "main span", ?pid);
    let _entered = span.enter();

    let uid = getuid().as_raw();
    let gid = getgid().as_raw();
    debug!(?uid, ?gid);

    let pid = getpid();
    unshare(CloneFlags::CLONE_NEWUSER | CloneFlags::CLONE_NEWNS)?;

    {
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/proc/{}/uid_map", pid.as_raw()))?;
        let msg = format!("0 {uid} 1");
        write!(&mut f, "{msg}")?;
    }

    {
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/proc/{}/setgroups", pid.as_raw()))?;
        write!(&mut f, "deny")?;
    }

    {
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/proc/{}/gid_map", pid.as_raw()))?;
        let msg = format!("0 {gid} 1");
        write!(&mut f, "{msg}")?;
    }

    let target = PathBuf::from(std::env::var("PWD")?);
    let prefix = "/home/ayats/.cache/hover-rs";

    // mount(
    //     Some("tmpfs"),
    //     "/home/ayats/.cache/hover-rs/layer",
    //     Some("tmpfs"),
    //     MsFlags::empty(),
    //     NNONE,
    // )?;

    mount(
        Some(&target),
        "/home/ayats/.cache/hover-rs/home",
        NNONE,
        MsFlags::MS_BIND,
        NNONE,
    )?;

    mount(
        NNONE,
        "/home/ayats/.cache/hover-rs/home",
        NNONE,
        MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY,
        NNONE,
    )?;

    mount(
        Some("overlay"),
        format!("{prefix}/newroot").as_str(),
        Some("overlay"),
        MsFlags::empty(),
        Some(format!("lowerdir={prefix}/home,upperdir={prefix}/layer,workdir={prefix}/work").as_str()),
    )?;
    // mount(
    //     Some("overlay"),
    //     "/home/ayats/.cache/hover-rs/newroot",
    //     Some("overlay"),
    //     MsFlags::empty(),
    //     Some(
    //         format!("lowerdir={prefix}/home,upperdir={prefix}/layer,workdir={prefix}/work")
    //             .as_str(),
    //     ),
    // )?;

    std::process::Command::new("/run/current-system/sw/bin/bash").exec();

    Ok(())
}
