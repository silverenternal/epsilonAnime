use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize)]
struct Envelope<T> {
    version: u32,
    payload: T,
}

pub fn state_dir() -> PathBuf {
    if let Ok(path) = std::env::var("EPSILON_ANIME_STATE_DIR")
        .or_else(|_| std::env::var("HAPPYBIRTHDAY_STATE_DIR"))
    {
        return PathBuf::from(path);
    }
    let base = if cfg!(windows) {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
    } else if let Ok(path) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(path)
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join(".config")
    };
    migrate_legacy_state(&base)
}

fn migrate_legacy_state(base: &Path) -> PathBuf {
    let current = base.join("epsilonAnime");
    let legacy = base.join("happybirthday");
    if !current.exists() && legacy.exists() && fs::rename(&legacy, &current).is_err() {
        return legacy;
    }
    current
}

pub fn load<T>(name: &str, version: u32) -> T
where
    T: Default + DeserializeOwned + Serialize,
{
    load_from(&state_dir(), name, version)
}

pub fn load_optional<T>(name: &str, version: u32) -> Option<T>
where
    T: DeserializeOwned + Serialize,
{
    load_or_migrate(&state_dir(), name, version)
}

fn load_from<T>(root: &Path, name: &str, version: u32) -> T
where
    T: Default + DeserializeOwned + Serialize,
{
    load_or_migrate(root, name, version).unwrap_or_default()
}

fn load_or_migrate<T>(root: &Path, name: &str, version: u32) -> Option<T>
where
    T: DeserializeOwned + Serialize,
{
    let path = root.join(name);
    if let Ok(bytes) = fs::read(&path) {
        match serde_json::from_slice::<Envelope<serde_json::Value>>(&bytes) {
            Ok(envelope) if envelope.version == version => {
                return match serde_json::from_value(envelope.payload) {
                    Ok(payload) => Some(payload),
                    Err(_) => {
                        backup_corrupt(&path);
                        None
                    }
                };
            }
            Ok(envelope) if envelope.version < version => {
                return match serde_json::from_value::<T>(envelope.payload) {
                    Ok(payload) => {
                        let _ = save_to(root, name, &payload, version);
                        Some(payload)
                    }
                    Err(_) => {
                        backup_corrupt(&path);
                        None
                    }
                };
            }
            Ok(_) | Err(_) => {
                backup_corrupt(&path);
                return None;
            }
        }
    }
    let legacy = Path::new("target").join(name);
    if !path.exists()
        && legacy.exists()
        && let Ok(bytes) = fs::read(&legacy)
        && let Ok(payload) = serde_json::from_slice::<T>(&bytes)
    {
        let _ = save_to(root, name, &payload, version);
        return Some(payload);
    }
    None
}

pub fn save<T: Serialize>(name: &str, payload: &T, version: u32) {
    if cfg!(test) {
        return;
    }
    let _ = save_to(&state_dir(), name, payload, version);
}

fn save_to<T: Serialize>(
    root: &Path,
    name: &str,
    payload: &T,
    version: u32,
) -> std::io::Result<()> {
    fs::create_dir_all(root)?;
    let target = root.join(name);
    let temp = root.join(format!("{name}.{}.tmp", std::process::id()));
    let text =
        serde_json::to_vec_pretty(&Envelope { version, payload }).map_err(std::io::Error::other)?;
    {
        let mut file = fs::File::create(&temp)?;
        file.write_all(&text)?;
        file.sync_all()?;
    }
    match replace_file(&temp, &target) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temp);
            Err(error)
        }
    }
}

#[cfg(not(windows))]
fn replace_file(temp: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(temp, target)
}

#[cfg(windows)]
fn replace_file(temp: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    let from: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn backup_corrupt(path: &Path) {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    let _ = fs::rename(path, path.with_extension(format!("corrupt.{stamp}")));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Sample {
        value: u32,
    }

    fn temp_root(label: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("epsilonanime-state-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn incompatible_schema_is_backed_up_and_defaults() {
        let root = temp_root("corrupt");
        fs::write(
            root.join("sample.json"),
            br#"{"version":1,"payload":{"wrong":"shape"}}"#,
        )
        .unwrap();
        let loaded: Sample = load_from(&root, "sample.json", 2);
        assert_eq!(loaded, Sample::default());
        assert!(fs::read_dir(&root).unwrap().any(|entry| {
            entry
                .ok()
                .is_some_and(|entry| entry.file_name().to_string_lossy().contains("corrupt"))
        }));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn compatible_older_schema_is_migrated_in_place() {
        let root = temp_root("migration");
        fs::write(
            root.join("sample.json"),
            br#"{"version":1,"payload":{"value":9}}"#,
        )
        .unwrap();
        let loaded: Sample = load_from(&root, "sample.json", 3);
        assert_eq!(loaded.value, 9);
        let envelope: Envelope<serde_json::Value> =
            serde_json::from_slice(&fs::read(root.join("sample.json")).unwrap()).unwrap();
        assert_eq!(envelope.version, 3);
        assert_eq!(envelope.payload["value"], 9);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn atomic_save_round_trips_without_leaving_temp_files() {
        let root = temp_root("atomic");
        let sample = Sample { value: 42 };
        save_to(&root, "sample.json", &sample, 1).unwrap();
        assert_eq!(load_from::<Sample>(&root, "sample.json", 1).value, 42);
        assert!(fs::read_dir(&root).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".tmp")
        }));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_state_directory_is_migrated_without_data_loss() {
        let base = temp_root("legacy-base");
        let legacy = base.join("happybirthday");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("playback.json"), b"legacy").unwrap();

        let migrated = migrate_legacy_state(&base);

        assert_eq!(migrated, base.join("epsilonAnime"));
        assert_eq!(fs::read(migrated.join("playback.json")).unwrap(), b"legacy");
        assert!(!legacy.exists());
        let _ = fs::remove_dir_all(base);
    }
}
