use rust_zip_archive::archive::EntryInfo;
use std::collections::BTreeMap;

/// One row in the file-manager view: a file or a (possibly synthesized) folder.
pub struct Node {
    pub name: String,
    pub full_path: String,
    pub is_dir: bool,
    pub size: u64,
    pub compressed: u64,
}

/// Immediate children of `dir` ("" = root): folders first, then files, each
/// group sorted by name. Folders are synthesized from path prefixes.
pub fn children(entries: &[EntryInfo], dir: &str) -> Vec<Node> {
    let prefix = if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    };

    let mut dirs: BTreeMap<String, ()> = BTreeMap::new();
    let mut files: Vec<Node> = Vec::new();

    for e in entries {
        // Normalize trailing slash on explicit directory entries.
        let path = e.name.trim_end_matches('/');
        if path.is_empty() {
            continue;
        }
        let Some(rest) = path.strip_prefix(&prefix) else {
            continue;
        };
        if rest.is_empty() {
            continue; // the dir entry itself
        }
        match rest.split_once('/') {
            Some((first, _)) => {
                // `first` is a subfolder directly under `dir`.
                dirs.insert(first.to_string(), ());
            }
            None => {
                if e.name.ends_with('/') {
                    dirs.insert(rest.to_string(), ());
                } else {
                    files.push(Node {
                        name: rest.to_string(),
                        full_path: format!("{prefix}{rest}"),
                        is_dir: false,
                        size: e.size,
                        compressed: e.compressed,
                    });
                }
            }
        }
    }

    let mut out: Vec<Node> = dirs
        .into_keys()
        .map(|name| Node {
            full_path: format!("{prefix}{name}"),
            name,
            is_dir: true,
            size: 0,
            compressed: 0,
        })
        .collect();
    files.sort_by(|a, b| a.name.cmp(&b.name));
    out.extend(files);
    out
}

#[cfg(test)]
mod tests {
    use super::children;
    use rust_zip_archive::archive::EntryInfo;

    fn e(name: &str, is_dir: bool) -> EntryInfo {
        EntryInfo {
            name: name.to_string(),
            size: 10,
            compressed: 5,
            is_dir,
        }
    }

    #[test]
    fn root_lists_top_level_folders_first() {
        let entries = vec![
            e("top.txt", false),
            e("a/b.txt", false),
            e("a/c/d.txt", false),
        ];
        let kids = children(&entries, "");
        let names: Vec<_> = kids.iter().map(|n| (n.name.as_str(), n.is_dir)).collect();
        assert_eq!(names, vec![("a", true), ("top.txt", false)]);
        assert_eq!(kids[0].full_path, "a");
    }

    #[test]
    fn subdir_lists_its_children() {
        let entries = vec![e("a/b.txt", false), e("a/c/d.txt", false)];
        let kids = children(&entries, "a");
        let names: Vec<_> = kids.iter().map(|n| (n.name.as_str(), n.is_dir)).collect();
        assert_eq!(names, vec![("c", true), ("b.txt", false)]);
        assert_eq!(kids[0].full_path, "a/c");
    }

    #[test]
    fn explicit_dir_entries_are_not_duplicated() {
        let entries = vec![e("a/", true), e("a/b.txt", false)];
        let kids = children(&entries, "");
        assert_eq!(kids.len(), 1);
        assert_eq!((kids[0].name.as_str(), kids[0].is_dir), ("a", true));
    }
}
