//! Opens the system browser for the ChatGPT sign-in (#31).
//!
//! An AppImage starts Splitr with its bundled libraries on `LD_LIBRARY_PATH` and its own `PATH`, GIO
//! and GTK settings. Anything Splitr starts inherits them, so `xdg-open` and the browser can fail
//! against libraries they weren't built for, often after the launcher has reported success. Only
//! the launched program gets the cleaned settings; Splitr's own and the system's stay as they are.

use std::ffi::OsString;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

/// How long a launcher is watched for an immediate failure before it counts as handed off.
const FAILURE_WINDOW: Duration = Duration::from_millis(500);

/// Set by the AppImage's launcher, with nothing from the user's own settings in them.
const APPIMAGE_ONLY: &[&str] = &["APPDIR", "APPIMAGE", "ARGV0", "OWD", "GTK_THEME", "PYTHONDONTWRITEBYTECODE"];

/// Tries to open `url`. Linux launchers can report success with no browser appearing, so the
/// sign-in always shows the link as well and nothing here is treated as final.
pub fn open(app: &AppHandle, url: &str) {
    let appimage = std::env::var_os("APPIMAGE").and(std::env::var("APPDIR").ok());
    match appimage {
        Some(appdir) => {
            let vars = outside_appimage(std::env::vars_os(), &appdir);
            let commands = open::commands(url).into_iter().map(|mut command| {
                command.env_clear().envs(vars.iter().cloned());
                command
            });
            launch(commands);
        }
        None => {
            let _ = app.opener().open_url(url, None::<&str>);
        }
    }
}

/// The environment without what the AppImage's launcher added: paths inside `appdir` are removed
/// from each variable, and a variable left with nothing is dropped.
fn outside_appimage(vars: impl IntoIterator<Item = (OsString, OsString)>, appdir: &str) -> Vec<(OsString, OsString)> {
    let inside = |entry: &str| entry.strip_prefix(appdir).is_some_and(|rest| rest.is_empty() || rest.starts_with('/'));
    vars.into_iter()
        .filter(|(name, _)| !name.to_str().is_some_and(|name| APPIMAGE_ONLY.contains(&name)))
        .filter_map(|(name, value)| {
            let Some(text) = value.to_str().filter(|text| text.contains(appdir)) else {
                return Some((name, value));
            };
            let kept: Vec<&str> = text.split(':').filter(|entry| !entry.is_empty() && !inside(entry)).collect();
            (!kept.is_empty()).then(|| (name, OsString::from(kept.join(":"))))
        })
        .collect()
}

/// Runs the launchers in order until one exits cleanly or is still running after the failure
/// window. False if all of them failed.
fn launch(commands: impl IntoIterator<Item = Command>) -> bool {
    commands.into_iter().any(|mut command| {
        let spawned = command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
        let Ok(mut child) = spawned else { return false };
        let start = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return status.success(),
                Err(_) => return false,
                Ok(None) if start.elapsed() >= FAILURE_WINDOW => {
                    // xdg-open can stay until the browser closes. Collect it then.
                    std::thread::spawn(move || child.wait());
                    return true;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(list: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        list.iter().map(|(name, value)| ((*name).into(), (*value).into())).collect()
    }

    const APPDIR: &str = "/tmp/.mount_Splitr1";

    #[test]
    fn the_appimage_paths_are_removed_and_the_users_are_kept() {
        let cleaned = outside_appimage(
            vars(&[
                ("PATH", "/tmp/.mount_Splitr1/usr/bin/:/tmp/.mount_Splitr1/bin/:/usr/local/bin:/usr/bin"),
                ("XDG_DATA_DIRS", "/tmp/.mount_Splitr1/usr/share/:/usr/share:/var/lib/flatpak/exports/share"),
                ("HOME", "/home/a"),
            ]),
            APPDIR,
        );
        assert_eq!(
            cleaned,
            vars(&[
                ("PATH", "/usr/local/bin:/usr/bin"),
                ("XDG_DATA_DIRS", "/usr/share:/var/lib/flatpak/exports/share"),
                ("HOME", "/home/a"),
            ])
        );
    }

    #[test]
    fn a_variable_with_only_appimage_paths_is_dropped() {
        // The launcher ends these with ":" and whatever was set before, which an unset variable leaves empty.
        let cleaned = outside_appimage(
            vars(&[
                ("LD_LIBRARY_PATH", "/tmp/.mount_Splitr1/usr/lib/:/tmp/.mount_Splitr1/lib/:"),
                ("GIO_MODULE_DIR", "/tmp/.mount_Splitr1//usr/lib/gio/modules"),
                ("GDK_PIXBUF_MODULE_FILE", "/tmp/.mount_Splitr1//usr/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache"),
                ("GTK_DATA_PREFIX", "/tmp/.mount_Splitr1"),
            ]),
            APPDIR,
        );
        assert_eq!(cleaned, vars(&[]));
    }

    #[test]
    fn the_users_own_library_path_survives() {
        let cleaned = outside_appimage(vars(&[("LD_LIBRARY_PATH", "/tmp/.mount_Splitr1/usr/lib/:/opt/mine/lib")]), APPDIR);
        assert_eq!(cleaned, vars(&[("LD_LIBRARY_PATH", "/opt/mine/lib")]));
    }

    #[test]
    fn the_launchers_own_settings_are_dropped_and_lookalike_paths_kept() {
        let cleaned = outside_appimage(
            vars(&[
                ("APPDIR", "/tmp/.mount_Splitr1"),
                ("APPIMAGE", "/home/a/Splitr.AppImage"),
                ("ARGV0", "./Splitr.AppImage"),
                ("OWD", "/home/a"),
                ("GTK_THEME", "Adwaita:dark"),
                ("PYTHONDONTWRITEBYTECODE", "1"),
                ("MY_DIRS", "/tmp/.mount_Splitr12/lib:/opt/a"),
                ("LANG", "en_US.UTF-8"),
            ]),
            APPDIR,
        );
        assert_eq!(cleaned, vars(&[("MY_DIRS", "/tmp/.mount_Splitr12/lib:/opt/a"), ("LANG", "en_US.UTF-8")]));
    }

    /// This test binary: `--list` exits cleanly and an unknown flag fails.
    fn test_binary(arg: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.arg(arg);
        command
    }

    #[test]
    fn launchers_are_tried_until_one_works() {
        let missing = || Command::new("splitr-no-such-launcher");
        assert!(launch([missing(), test_binary("--no-such-flag"), test_binary("--list")]));
        assert!(!launch([missing(), test_binary("--no-such-flag")]));
        assert!(!launch([]));
    }
}
