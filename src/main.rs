mod utils;

use crate::utils::{callback_wrapper, NNONE};

use caps::{CapSet, Capability};
use eyre::{bail, ensure, Context};
use nix::errno::Errno;
use nix::libc::SIGCHLD;
use nix::mount::{mount, MsFlags};
use nix::sched::{clone, unshare, CloneFlags};
use nix::sys::prctl::set_pdeathsig;
use nix::sys::signal::Signal;
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::{close, isatty, Gid, Pid, Uid};
use owo_colors::OwoColorize;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::ffi::OsStringExt;
use std::process::Command;
use std::time::SystemTime;
use std::{env, fs};
use std::{os::unix::process::CommandExt, path::PathBuf};
use tracing::{debug, error};

#[derive(Debug, clap::Parser)]
/// hover-rs: protective home overlay
struct Args {
    /// Command and arguments to execute
    #[arg(trailing_var_arg = true)]
    command: Vec<OsString>,
}

#[derive(Debug, Clone)]
struct Config {
    target: PathBuf,
    runtime: PathBuf,
    cache: PathBuf,
    allocation: String,
    layer: PathBuf,
    _work: PathBuf, // Implementation of overlayfs
    uid: Uid,
    gid: Gid,
}

fn main() -> eyre::Result<()> {
    {
        color_eyre::install()?;
        use tracing_subscriber::{fmt, prelude::*, EnvFilter};
        tracing_subscriber::registry()
            .with(fmt::layer().without_time().with_line_number(true))
            .with(EnvFilter::from_default_env())
            .init();

        if env::var("HOVER").is_ok() {
            bail!("hover can't be stacked!");
        };
        env::set_var("HOVER", "1");
    }

    let args = <Args as clap::Parser>::parse();
    debug!(?args);

    let config = Config::build()?;

    ensure!(
        !config.uid.is_root(),
        "hover-rs is not made to be run as root!"
    );

    let (argv0, argv): (OsString, Vec<OsString>) = if args.command.is_empty() {
        if !isatty(libc::STDIN_FILENO )? {
            bail!("Not running as a tty, and no program provided, aborting");
        }

        let parent = Pid::parent();
        let parent_exe = fs::read_link(format!("/proc/{}/exe", parent))?;
        let mut cmdline = fs::read(format!("/proc/{}/cmdline", parent))?;
        cmdline.pop(); // removes trailing \0
        let parent_argv = cmdline
            .into_iter()
            .fold(Vec::new(), |mut acc, x| {
                if x == 0 {
                    acc.push(Vec::new());
                } else {
                    if acc.is_empty() {
                        acc.push(Vec::new());
                    }
                    acc.last_mut().unwrap().push(x);
                }
                acc
            })
            .into_iter()
            .map(OsString::from_vec);

        let mut parent_argv = parent_argv.collect::<Vec<_>>();
        parent_argv.remove(0); // remove argv0, we use exe
        debug!(?parent_exe, ?parent_argv);

        (parent_exe.as_os_str().to_owned(), parent_argv)
    } else {
        let mut _args = args.command.into_iter();
        (_args.next().unwrap(), _args.collect())
    };
    let mut cmd = Command::new(argv0);
    cmd.args(argv);

    let pipe = unsafe {
        let mut fds = [-1; 2];
        let ret = libc::pipe(fds.as_mut_ptr());
        if ret == -1 {
            return Err(Errno::last()).wrap_err("Failed to create pipe");
        }
        (fds[0], fds[1])
    };
    debug!(?pipe);

    let mut stack = [0; 4000];
    let child = unsafe {
        let config = config.clone();
        clone(
            Box::new(move || {
                callback_wrapper(|| -> eyre::Result<()> {
                    // Close writing pipe
                    close(pipe.1)?;

                    debug!("Waiting for parent...");
                    let mut dummy = [0];
                    libc::read(pipe.0, dummy.as_mut_ptr() as _, 1);
                    debug!("Parent done");

                    set_pdeathsig(Some(Signal::SIGTERM))?;
                    setup(&config)?;
                    let error = cmd.exec();
                    Err(error).wrap_err("Failed to execute the command")
                })
            }),
            &mut stack,
            CloneFlags::CLONE_NEWUSER | CloneFlags::CLONE_NEWNS,
            Some(SIGCHLD),
        )
    }?;

    // Close reading pipe
    close(pipe.0)?;

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
        let msg = format!("0 {} 1", config.uid);
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
        let msg = format!("0 {} 1", config.gid);
        f.write(msg.as_bytes())
            .wrap_err("Setting gid_map for child process")?;
    }

    println!("You are now {}", "hovering~".bold());
    println!(
        "  A layer is covering your {}",
        config.target.to_string_lossy().bold().red()
    );
    println!(
        "  You can find your top layer in: {}",
        config.layer.to_string_lossy().bold().red()
    );

    // Close writing pipe. Setup is done
    close(pipe.1)?;

    let r#return = waitpid(child, None)?;
    if let WaitStatus::Exited(_, 0) = r#return {
        debug!(?r#return);
    } else {
        error!(?r#return);
    }

    println!("Leaving {}", "hover".bold());
    println!(
        "  You can find your top layer in: {}",
        config.layer.to_string_lossy().bold().red()
    );

    Ok(())
}

fn setup(config: &Config) -> eyre::Result<()> {
    mount(
        Some("tmpfs"),
        &config.runtime,
        Some("tmpfs"),
        MsFlags::empty(),
        NNONE,
    )?;

    let ro_mount = config.runtime.join("oldroot");
    fs::create_dir_all(&ro_mount)?;

    // Mount root dir as RO
    mount(
        Some(&config.target),
        &ro_mount,
        NNONE,
        MsFlags::MS_BIND,
        NNONE,
    )?;
    mount(
        NNONE,
        &ro_mount,
        NNONE,
        MsFlags::MS_BIND | MsFlags::MS_REMOUNT | MsFlags::MS_RDONLY,
        NNONE,
    )?;

    {
        // Don't use format! because the paths might not be valid str, keep OsStr's
        let mut opts = OsString::from("lowerdir=");
        opts.push(ro_mount.as_os_str());
        opts.push(",upperdir=");
        opts.push(&config.layer);
        opts.push(",workdir=");
        opts.push(&config._work);

        mount(
            Some("overlay"),
            &config.target,
            Some("overlay"),
            MsFlags::empty(),
            Some(opts.as_os_str()),
        )?;
    }

    // Workdir is under the overlay
    env::set_current_dir(env::current_dir()?)?;

    // Seal the working dirs from the user
    mount(
        Some("/var/empty"),
        &config.runtime,
        NNONE,
        MsFlags::MS_BIND,
        NNONE,
    )?;

    mount(
        Some("/var/empty"),
        &config.cache,
        NNONE,
        MsFlags::MS_BIND,
        NNONE,
    )?;

    // Map back to original user
    unshare(CloneFlags::CLONE_NEWUSER)?;

    {
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/proc/self/uid_map")?;
        let msg = format!("{} 0 1", config.uid);
        f.write(msg.as_bytes())?;
    }
    {
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/proc/self/gid_map")?;
        let msg = format!("{} 0 1", config.gid);
        f.write(msg.as_bytes())?;
    }

    Ok(())
}

impl Config {
    fn build() -> eyre::Result<Self> {
        let cache = env::var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| env::var("HOME").map(PathBuf::from).unwrap().join(".cache"))
            .join("hover-rs");
        std::fs::create_dir_all(&cache)?;

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

        let runtime = env::var("XDG_RUNTIME_DIR")
            .map(|s| PathBuf::from(s).join("hover-rs"))
            .unwrap_or_else(|_| PathBuf::from("/tmp").join(format!("hover-rs-{allocation}")));
        std::fs::create_dir_all(&runtime)?;

        let layer = cache.join(format!("layer-{allocation}"));
        fs::create_dir_all(&layer)?;
        let _work = cache.join(format!(".work-{allocation}"));
        fs::create_dir_all(&_work)?;

        Ok(Self {
            target: PathBuf::from(env::var("HOME")?),
            runtime,
            cache,
            allocation,
            layer,
            _work,
            uid: Uid::current(),
            gid: Gid::current(),
        })
    }
}
