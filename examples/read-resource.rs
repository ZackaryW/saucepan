//! External consumer: Saucepan owns lifetime; the tool only reads data.
use std::{
    env, fs,
    path::PathBuf,
    time::{Duration, Instant},
};

fn main() -> anyhow::Result<()> {
    let resource = PathBuf::from(
        env::var_os("SAUCEPAN_RESOURCE_DIR")
            .ok_or_else(|| anyhow::anyhow!("run this tool through saucepan run"))?,
    );
    // Optional handshake lets an acceptance caller exercise concurrent reclaim.
    if let Some(ready) = env::var_os("CONSUMER_READY") {
        let release = PathBuf::from(
            env::var_os("CONSUMER_RELEASE")
                .ok_or_else(|| anyhow::anyhow!("CONSUMER_RELEASE required with CONSUMER_READY"))?,
        );
        fs::write(ready, b"ready")?;
        let start = Instant::now();
        while !release.exists() {
            anyhow::ensure!(
                start.elapsed() < Duration::from_secs(60),
                "consumer handshake timed out"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    print!("{}", fs::read_to_string(resource.join("data"))?);
    Ok(())
}
