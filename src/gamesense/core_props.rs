//! `coreProps.json`: how games find the GameSense server.
//!
//! Games read `%PROGRAMDATA%/SteelSeries/SteelSeries Engine 3/coreProps.json` and post to its
//! `address`. On Linux most games run under Wine or Proton, so the file is written into every
//! existing Wine/Proton prefix as well as `/tmp/steelseries-engine/`.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use tracing::{debug, warn};

#[cfg(unix)]
use crate::Error;
use crate::Result;

/// File name games look for.
pub const CORE_PROPS_FILE: &str = "coreProps.json";

/// Folders under `ProgramData` where SteelSeries Engine keeps `coreProps.json`.
const ENGINE_DIRS: [&str; 2] = ["SteelSeries", "SteelSeries Engine 3"];

/// Most entries read from one directory while scanning for prefixes.
const MAX_DIR_ENTRIES: usize = 4096;

/// Body of `coreProps.json` for a server on `port`.
pub fn core_props_json(port: u16) -> serde_json::Value {
    serde_json::json!({
        "address": format!("127.0.0.1:{port}"),
        "encrypted_address": "",
        "ggEncryptedAddress": "",
    })
}

/// The native location on Windows: `%PROGRAMDATA%\SteelSeries\SteelSeries Engine 3\coreProps.json`.
#[cfg(target_os = "windows")]
pub fn system_core_props_path() -> Option<PathBuf> {
    let program_data = std::env::var_os("PROGRAMDATA")?;
    Some(engine_file(Path::new(&program_data)))
}

/// The native location on macOS.
#[cfg(target_os = "macos")]
pub fn system_core_props_path() -> Option<PathBuf> {
    Some(PathBuf::from("/Library/Application Support/SteelSeries Engine 3").join(CORE_PROPS_FILE))
}

/// The native location on Linux: `/tmp/steelseries-engine/coreProps.json`.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn system_core_props_path() -> Option<PathBuf> {
    Some(PathBuf::from("/tmp/steelseries-engine").join(CORE_PROPS_FILE))
}

fn engine_file(program_data: &Path) -> PathBuf {
    program_data
        .join(ENGINE_DIRS[0])
        .join(ENGINE_DIRS[1])
        .join(CORE_PROPS_FILE)
}

/// Every location to write: the native path, plus Wine/Proton prefixes under `home` on Linux.
pub fn core_props_targets(home: Option<&Path>) -> Vec<PathBuf> {
    let mut targets: Vec<PathBuf> = system_core_props_path().into_iter().collect();
    if cfg!(target_os = "linux") {
        if let Some(home) = home {
            targets.extend(discover_prefix_core_props(home));
        }
    }
    targets
}

/// `coreProps.json` paths inside every existing Wine/Proton prefix under `home`.
///
/// A prefix counts when its `drive_c/ProgramData` folder exists; nothing is created here.
/// Scanned locations:
/// - Steam (native, `~/.steam`, Flatpak) `steamapps/compatdata/*/pfx`, including every library
///   listed in `libraryfolders.vdf`
/// - `~/.wine`
/// - Lutris games in `~/Games/*` (prefix at the game folder or its `pfx`)
/// - Heroic in `~/Games/Heroic/Prefixes/*` and `~/Games/Heroic/Prefixes/default/*`
/// - Bottles (native and Flatpak) `bottles/*`
pub fn discover_prefix_core_props(home: &Path) -> Vec<PathBuf> {
    let mut program_data_dirs: Vec<PathBuf> = Vec::new();

    for steamapps in steamapps_dirs(home) {
        for prefix in subdirs(&steamapps.join("compatdata")) {
            program_data_dirs.push(prefix.join("pfx").join("drive_c").join("ProgramData"));
        }
    }

    program_data_dirs.push(home.join(".wine").join("drive_c").join("ProgramData"));

    let prefix_parents = [
        home.join("Games"),
        home.join("Games").join("Heroic").join("Prefixes"),
        home.join("Games").join("Heroic").join("Prefixes").join("default"),
        home.join(".local").join("share").join("bottles").join("bottles"),
        home.join(".var/app/com.usebottles.bottles/data/bottles/bottles"),
    ];
    for parent in &prefix_parents {
        for prefix in subdirs(parent) {
            program_data_dirs.push(prefix.join("drive_c").join("ProgramData"));
            program_data_dirs.push(prefix.join("pfx").join("drive_c").join("ProgramData"));
        }
    }

    let mut seen = HashSet::new();
    let mut targets = Vec::new();
    for dir in program_data_dirs {
        if !dir.is_dir() {
            continue;
        }
        let key = fs::canonicalize(&dir).unwrap_or_else(|_| dir.clone());
        if seen.insert(key) {
            targets.push(engine_file(&dir));
        }
    }
    targets
}

/// `steamapps` folders of every Steam install and library under `home`.
fn steamapps_dirs(home: &Path) -> Vec<PathBuf> {
    let roots = [
        home.join(".steam").join("steam"),
        home.join(".steam").join("root"),
        home.join(".local").join("share").join("Steam"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        home.join(".var/app/com.valvesoftware.Steam/data/Steam"),
    ];
    let mut dirs: Vec<PathBuf> = Vec::new();
    for root in &roots {
        dirs.push(root.join("steamapps"));
        for vdf in [
            root.join("steamapps").join("libraryfolders.vdf"),
            root.join("config").join("libraryfolders.vdf"),
        ] {
            if let Ok(text) = fs::read_to_string(&vdf) {
                dirs.extend(
                    parse_library_folders(&text)
                        .into_iter()
                        .map(|lib| lib.join("steamapps")),
                );
            }
        }
    }
    let mut seen = HashSet::new();
    dirs.retain(|d| d.is_dir() && seen.insert(fs::canonicalize(d).unwrap_or_else(|_| d.clone())));
    dirs
}

/// Library paths from Steam's `libraryfolders.vdf` (every `"path"` value).
pub fn parse_library_folders(vdf: &str) -> Vec<PathBuf> {
    let tokens = vdf_strings(vdf);
    tokens
        .windows(2)
        .filter(|pair| pair[0].eq_ignore_ascii_case("path"))
        .map(|pair| PathBuf::from(&pair[1]))
        .collect()
}

/// Quoted strings of a VDF (KeyValues) file, with `\\` and `\"` unescaped.
fn vdf_strings(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut s = String::new();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => match chars.next() {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some(other) => s.push(other),
                    None => break,
                },
                other => s.push(other),
            }
        }
        out.push(s);
    }
    out
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .take(MAX_DIR_ENTRIES)
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    out.sort();
    out
}

/// Write `coreProps.json` for `port` to the native location and into every existing
/// Wine/Proton prefix of the current user. Returns the files written.
///
/// Locations that fail are logged and skipped; an error is returned only when every
/// location failed.
pub fn write_core_props(port: u16) -> Result<Vec<PathBuf>> {
    let home = directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf());
    write_core_props_to(port, &core_props_targets(home.as_deref()))
}

/// Write `coreProps.json` for `port` to each of `targets`, creating the parent folders.
/// Returns the files written; an error only when every target failed.
pub fn write_core_props_to(port: u16, targets: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let content = core_props_json(port);
    let mut written = Vec::with_capacity(targets.len());
    let mut last_error = None;
    for target in targets {
        match write_secure_json(target, &content) {
            Ok(()) => written.push(target.clone()),
            Err(e) => {
                warn!("GameSense: could not write {}: {}", target.display(), e);
                last_error = Some(e);
            }
        }
    }
    match last_error {
        Some(e) if written.is_empty() => Err(e),
        _ => Ok(written),
    }
}

/// Remove `coreProps.json` files written by [`write_core_props`], and the SteelSeries folders
/// around them when they are left empty.
pub fn remove_core_props(paths: &[PathBuf]) {
    for path in paths {
        match fs::remove_file(path) {
            Ok(()) => debug!("GameSense: removed {}", path.display()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                warn!("GameSense: could not remove {}: {}", path.display(), e);
                continue;
            }
        }
        let mut dir = path.parent();
        while let Some(d) = dir {
            let removable = d
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| ENGINE_DIRS.contains(&n) || n == "steelseries-engine");
            // remove_dir only succeeds on empty folders, which is the intent.
            if !removable || fs::remove_dir(d).is_err() {
                break;
            }
            dir = d.parent();
        }
    }
}

/// Removes the listed files when dropped, so they go away however the server stops.
#[derive(Debug, Default)]
pub(crate) struct CorePropsGuard {
    paths: Vec<PathBuf>,
}

impl CorePropsGuard {
    pub(crate) fn new(paths: Vec<PathBuf>) -> Self {
        Self { paths }
    }
}

impl Drop for CorePropsGuard {
    fn drop(&mut self) {
        remove_core_props(&self.paths);
    }
}

/// Write JSON with owner-only permissions, refusing symlinks and folders owned by others.
pub(crate) fn write_secure_json(path: &Path, content: &serde_json::Value) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};

        if let Some(parent) = path.parent() {
            if parent.exists() {
                let metadata = fs::symlink_metadata(parent)?;
                if !metadata.is_dir() {
                    return Err(Error::GameSense(format!(
                        "Security error: {:?} is not a directory",
                        parent
                    )));
                }
                if metadata.uid() != rustix::process::getuid().as_raw() {
                    return Err(Error::GameSense(format!(
                        "Security error: {:?} is not owned by the current user",
                        parent
                    )));
                }
            } else {
                fs::DirBuilder::new().recursive(true).mode(0o700).create(parent)?;
            }
        }

        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        options.mode(0o600);
        options.custom_flags(libc::O_NOFOLLOW);
        let file = options.open(path)?;

        let mut perms = file.metadata()?.permissions();
        perms.set_mode(0o600);
        file.set_permissions(perms)?;

        serde_json::to_writer_pretty(file, content)?;
        debug!("Wrote secure JSON to {:?}", path);
    }

    #[cfg(not(unix))]
    {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(content)?;
        crate::fs_utils::secure_write(path, json.as_bytes())?;
        debug!("Wrote JSON to {:?}", path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_props_content() {
        let v = core_props_json(27301);
        assert_eq!(v["address"], "127.0.0.1:27301");
        assert_eq!(v["encrypted_address"], "");
        assert_eq!(v["ggEncryptedAddress"], "");
    }

    #[test]
    fn library_folders_parsing() {
        let vdf = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"/home/user/.local/share/Steam"
		"label"		""
		"apps"
		{
			"228980"		"0"
		}
	}
	"1"
	{
		"path"		"/mnt/games/Steam \"Library\""
	}
}
"#;
        assert_eq!(
            parse_library_folders(vdf),
            vec![
                PathBuf::from("/home/user/.local/share/Steam"),
                PathBuf::from("/mnt/games/Steam \"Library\"")
            ]
        );
    }
}
