//! Upgrade cleanup is limited to YUME's own retired tool, never external clients or settings.
use std::fs;
use std::io;
use std::path::Path;

const RETIRED_TOOL: &str = "ccswitch_prepare_opencode_provider.ts";

fn reject_link(metadata: &fs::Metadata) -> io::Result<()> {
    let mut linked = metadata.file_type().is_symlink();
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        linked |= metadata.file_attributes() & 0x400 != 0;
    }
    if linked {
        return Err(io::Error::other("retired tool path must not be a link"));
    }
    Ok(())
}

pub(crate) fn retire_external_deploy_tool(data_dir: &Path) -> io::Result<()> {
    let mut directory = data_dir.to_path_buf();
    for component in ["", "workspace", ".opencode", "tools"] {
        directory.push(component);
        let metadata = match fs::symlink_metadata(&directory) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        reject_link(&metadata)?;
        if !metadata.is_dir() {
            return Err(io::Error::other("retired tool parent must be a directory"));
        }
    }
    let source = directory.join(RETIRED_TOOL);
    let metadata = match fs::symlink_metadata(&source) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    reject_link(&metadata)?;
    if !metadata.is_file() {
        return Err(io::Error::other("retired tool must be a regular file"));
    }
    let archive = data_dir.join("retired-tools");
    match fs::create_dir(&archive) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let metadata = fs::symlink_metadata(&archive)?;
    reject_link(&metadata)?;
    if !metadata.is_dir() {
        return Err(io::Error::other("retired tool archive must be a directory"));
    }
    // Preserve user edits outside all tool discovery paths; repeated launches do nothing.
    fs::rename(
        source,
        archive.join(format!("{}.ts.disabled", uuid::Uuid::new_v4())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrade_retires_only_the_old_tool_and_preserves_its_bytes() {
        let root = tempfile::tempdir().unwrap();
        let tools = root.path().join("workspace/.opencode/tools");
        fs::create_dir_all(&tools).unwrap();
        fs::write(tools.join(RETIRED_TOOL), "user-edited old tool").unwrap();
        fs::write(tools.join("memory_manage.ts"), "keep").unwrap();
        let snapshot = root.path().join("ccswitch-recovery/snapshot.json");
        fs::create_dir_all(snapshot.parent().unwrap()).unwrap();
        fs::write(&snapshot, "encrypted recovery").unwrap();
        retire_external_deploy_tool(root.path()).unwrap();
        retire_external_deploy_tool(root.path()).unwrap();
        assert!(!tools.join(RETIRED_TOOL).exists());
        assert_eq!(
            fs::read_to_string(tools.join("memory_manage.ts")).unwrap(),
            "keep"
        );
        assert_eq!(fs::read_to_string(snapshot).unwrap(), "encrypted recovery");
        let archived: Vec<_> = fs::read_dir(root.path().join("retired-tools"))
            .unwrap()
            .collect();
        assert_eq!(archived.len(), 1);
        assert_eq!(
            fs::read_to_string(archived[0].as_ref().unwrap().path()).unwrap(),
            "user-edited old tool"
        );
    }

    #[test]
    fn clean_install_creates_no_retirement_state() {
        let root = tempfile::tempdir().unwrap();
        retire_external_deploy_tool(root.path()).unwrap();
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn unexpected_tool_directory_is_not_moved_or_deleted() {
        let root = tempfile::tempdir().unwrap();
        let path = root
            .path()
            .join("workspace/.opencode/tools")
            .join(RETIRED_TOOL);
        fs::create_dir_all(&path).unwrap();
        assert!(retire_external_deploy_tool(root.path()).is_err());
        assert!(path.is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_linked_tool_directory() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("workspace")).unwrap();
        assert!(retire_external_deploy_tool(root.path()).is_err());
    }
}
