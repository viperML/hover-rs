use rand::{distributions::Alphanumeric, Rng};
use std::path::PathBuf;
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
            .with(fmt::layer().without_time())
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

    std::fs::create_dir_all(tmp_path)?;

    Ok(())
}
