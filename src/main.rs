use nix::{
    sched::{clone, unshare, CloneFlags},
    sys::{signal::Signal, wait::{wait, waitid, waitpid, WaitPidFlag}},
    unistd::{setuid, Pid, Uid},
};
use rand::{distributions::Alphanumeric, Rng};
use std::{fs::File, os::unix::process::CommandExt, path::PathBuf};
use std::{io::Write, time::Duration};
use tracing::{debug, info};

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

    let child = unsafe {
        clone(
            Box::new(callback),
            &mut stack,
                CloneFlags::CLONE_NEWUSER
                // CloneFlags::CLONE_VFORK
                ,
                // | CloneFlags::CLONE_VM,
            None,
        )?
    };
    debug!(?child);

    let mut uid_map = File::open(format!("/proc/{}/uid_map", child.as_raw()))?;
    write!(&mut uid_map, "0 1000 1").unwrap();

    // std::thread::sleep(Duration::from_secs(100000));
    waitpid(child, None)?;

    Ok(())
}

fn callback() -> isize {
    debug!("Hello from callback");

    let mycaps = caps::all();
    debug!("{:#?}", mycaps);

    for _ in 0..3 {
        let uid = Uid::current();
        let euid = Uid::effective();
        let pid = Pid::this();
        debug!(?uid, ?euid, ?pid);

        std::thread::sleep(Duration::from_secs(1));
    }

    return 0;
}
