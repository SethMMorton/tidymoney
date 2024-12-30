use std::convert::AsRef;
use std::fs;
use std::path::Path;

/// Move a file from one location to another.
fn move_file<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> std::io::Result<()> {
    match std::fs::rename(&from, &to) {
        Ok(result) => Ok(result),
        Err(_) => {
            std::fs::copy(&from, &to)?;
            std::fs::remove_file(from)?;
            Ok(())
        }
    }
}

/// Move transactions as downloaded into an "old" folder marked with a timestamp.
pub fn store_raw_transactions(
    storage: impl AsRef<Path>,
    files: &Vec<impl AsRef<Path>>,
    folder_base: impl AsRef<str>,
) -> std::io::Result<()> {
    // Create the location for the old locations.
    let location = storage.as_ref().join("old").join(folder_base.as_ref());
    if !location.exists() {
        fs::create_dir_all(&location)?;
    }

    // Move the files from the old to the new locations.
    for f in files {
        let name = f.as_ref().file_name().unwrap();
        move_file(f, location.join(name))?;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    use std::path::PathBuf;

    use tempdir;

    #[test]
    fn test_move_file() {
        // Create a file in a temporary directory with some text.
        let temp = tempdir::TempDir::new("test").unwrap();
        let msg = "some text\n";
        fs::write(temp.path().join("a.txt"), msg).unwrap();

        // Move the file.
        move_file(temp.path().join("a.txt"), temp.path().join("b.txt")).unwrap();

        // Read the new file and ensure the text matches the original file.
        let result = fs::read_to_string(temp.path().join("b.txt")).unwrap();
        assert_eq!(result, msg);

        // The old file must not exist.
        assert!(!fs::exists(temp.path().join("a.txt")).unwrap());
    }

    #[test]
    fn test_store_raw_transactions() {
        // Create a temp dir in which to perform the tests
        let temp = tempdir::TempDir::new("test").unwrap();
        let downloads = temp.path().join("downloads");
        fs::create_dir(&downloads).unwrap();

        // Create files in the downloads directory and store in a vector.
        let mut files: Vec<PathBuf> = Vec::new();
        for i in 1..5 {
            let path = downloads.join(format!("file{i}.csv"));
            fs::write(&path, "text").unwrap();
            files.push(path);
        }

        // Move the files.
        store_raw_transactions(temp.path(), &files, "base1").unwrap();

        // Check that the original files do not exist.
        for path in files {
            assert!(!fs::exists(path).unwrap());
        }

        // Check that the files have been moved.
        let base1 = temp.path().join("old").join("base1");
        for i in 1..5 {
            let path = base1.join(format!("file{i}.csv"));
            assert!(fs::exists(path).unwrap());
        }
    }
}
