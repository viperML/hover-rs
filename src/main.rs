use nix::mount::{mount, MsFlags};
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
use std::{fs::File, os::unix::process::CommandExt, path::PathBuf};
use std::{io::Write, time::Duration};
use tracing::{debug, info, span, Level};

#[derive(Debug, clap::Parser)]
struct Args {
    tmp_path: Option<PathBuf>,
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    {
        use tracing_subscriber::{fmt, prelude::*, EnvFilter};
        tracing_subscriber::registry()
            .with(fmt::layer())
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
            Box::new(callback),
            &mut stack,
                CloneFlags::CLONE_NEWUSER
                | CloneFlags::CLONE_NEWNS
                // CloneFlags::CLONE_VFORK
                ,
                // | CloneFlags::CLONE_VM,
            Some(SIGCHLD),
        )?
    };
    debug!(?child);

    {
        let mut uid_map = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/proc/{}/uid_map", child.as_raw()))?;

        debug!(?uid_map);
        write!(&mut uid_map, "0 1000 1\n")?;
    }

    // {
    //     let mut gid_map = OpenOptions::new()
    //         .read(true)
    //         .write(true)
    //         .open(format!("/proc/{}/gid_map", child.as_raw()))?;
    //
    //     debug!(?gid_map);
    //     write!(&mut gid_map, "0 100 1\n")?;
    // }

    let status = waitpid(child, None)?;
    debug!(?status);

    Ok(())
}

const NNONE: Option<&str> = None;

fn callback() -> isize {
    let pid = Pid::this();
    let span = span!(Level::DEBUG, "child span", ?pid);
    let _entered = span.enter();

    let ppid = Pid::parent();
    debug!(?ppid, "Hello from callback");

    mount(Some("tmpfs"), "/mnt", Some("tmpfs"), MsFlags::empty(), NNONE).unwrap();

    std::process::Command::new("/run/current-system/sw/bin/bash").exec();

    return 0;
}
