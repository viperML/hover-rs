#![allow(unused_imports)]
mod utils;

use nix::mount::{mount, MsFlags};
use nix::unistd::{getgid, getuid};
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

    let mut stack = [0; 2000];

    let pid = Pid::this();
    let span = span!(Level::DEBUG, "main span", ?pid);
    let _entered = span.enter();

    let child = unsafe {
        clone(
            Box::new(|| callback_wrapper(callback)),
            &mut stack,
            CloneFlags::CLONE_NEWUSER | CloneFlags::CLONE_NEWNS,
            Some(SIGCHLD),
        )?
    };
    debug!(?child);

    let uid = getuid().as_raw();
    let gid = getgid().as_raw();
    debug!(?uid, ?gid);

    {
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/proc/{}/uid_map", child.as_raw()))?;
        write!(&mut BufWriter::new(f), "0 {uid} 1")?;
    }

    {
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/proc/{}/setgroups", child.as_raw()))?;
        write!(&mut BufWriter::new(f), "deny")?;
    }

    {
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/proc/{}/gid_map", child.as_raw()))?;
        write!(&mut BufWriter::new(f), "0 {gid} 1")?;
    }

    let status = waitpid(child, None)?;
    debug!(?status);

    Ok(())
}


fn callback() -> eyre::Result<()> {
    let pid = Pid::this();
    let span = span!(Level::DEBUG, "child span", ?pid);
    let _entered = span.enter();

    let ppid = Pid::parent();
    debug!(?ppid, "Hello from callback");

    // TODO: sync when namespaces are ready
    std::thread::sleep(Duration::from_millis(200));

    let uid = Uid::current();
    let euid = Uid::effective();
    debug!(?uid, ?euid);

    let prefix = "/var/empty";
    let target = PathBuf::from(std::env::var("PWD")?);

    mount(
        Some("tmpfs"),
        prefix,
        Some("tmpfs"),
        MsFlags::empty(),
        NNONE,
    )?;

    mount(
        Some(&target),
        format!("/var/empty").as_str(),
        NNONE,
        MsFlags::MS_BIND,
        NNONE,
    )?;

    mount(
        NNONE,
        format!("/var/empty").as_str(),
        NNONE,
        MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY,
        NNONE,
    )?;

    std::process::Command::new("/run/current-system/sw/bin/bash").exec();

    Ok(())
}
