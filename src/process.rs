//! Starting other programs from a long-running process.

use std::path::PathBuf;

/// Spawn `cmd` and reap it on a background thread, so the child never lingers
/// as a zombie once it exits — for a launch the caller does not wait on
/// (`xdg-open`, a picker, a sibling app).
///
/// Twelve apps kept an identical private copy of this after cce-ui's
/// `process::spawn_detached` went away in cce-ui 4e94236; it lives here now.
pub fn spawn_detached(mut cmd: std::process::Command) -> std::io::Result<()> {
    let mut child = cmd.spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// A cce binary installed beside this one, else `name` for a PATH lookup.
///
/// A systemd user service runs with `PATH=/usr/local/bin:/usr/bin`, which
/// leaves out `~/.local/bin`, where every cce binary is installed, so a
/// bare-name spawn fails with ENOENT under systemd while working from a
/// shell. Looking beside the running executable finds the installed set,
/// and a workspace build's `target/` too; the PATH fallback covers a dev
/// build run on its own.
pub fn de_bin(name: &str) -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(name)))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn de_bin_falls_back_to_the_bare_name() {
        assert_eq!(de_bin("no-such-cce-binary"), PathBuf::from("no-such-cce-binary"));
    }

    #[test]
    fn a_detached_child_is_reaped() {
        let mut cmd = std::process::Command::new("true");
        cmd.stdout(std::process::Stdio::null());
        spawn_detached(cmd).unwrap();
    }
}
