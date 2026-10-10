//! Opt-in maintenance binary. Not bundled or registered as a Tauri command.
use std::path::PathBuf;
use yume_lib::legacy_ai_recovery::{
    RecoveryLocations, RecoveryManager, SnapshotId, SystemRecoveryKeyStore,
};

fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() != 7
        || args[0] != "--app-data"
        || args[2] != "--home"
        || args[4] != "--snapshot"
        || args[6] != "--restore"
    {
        return Err("Usage: recover-external-ai --app-data ABSOLUTE_PATH --home ABSOLUTE_PATH --snapshot UUID --restore\nRestores only the saved external OpenCode configuration; refuses changed files. Run as the original user with access to the original credential store.".into());
    }
    let data = PathBuf::from(&args[1]);
    let home = PathBuf::from(&args[3]);
    if !data.is_absolute() || !home.is_absolute() || !data.join("ccswitch-recovery").is_dir() {
        return Err("Existing absolute home and recovery data paths are required".into());
    }
    let snapshot = SnapshotId::parse(&args[5])?;
    let manager = RecoveryManager::new(RecoveryLocations::new(home, data), SystemRecoveryKeyStore)?;
    manager.restore(&snapshot)?;
    println!("External configuration restored. No credentials printed; external clients and auth files were not removed.");
    Ok(())
}

fn main() {
    if let Err(error) = run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mutation_requires_all_explicit_arguments() {
        assert!(run(&[]).is_err());
        assert!(run(&["--restore".into()]).is_err());
        assert!(run(&[
            "--app-data",
            "relative",
            "--home",
            "relative",
            "--snapshot",
            "bad",
            "--restore"
        ]
        .map(str::to_owned))
        .is_err());
    }
}
