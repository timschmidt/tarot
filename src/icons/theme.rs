//! Freedesktop theme precedence and size selection. No recursive tree scans.
use std::{
    collections::{HashMap, HashSet},
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Copy)]
enum Kind {
    Fixed,
    Scalable,
    Threshold,
}
#[derive(Clone)]
struct Directory {
    name: String,
    size: u32,
    scale: u32,
    min: u32,
    max: u32,
    kind: Kind,
}
impl Directory {
    fn distance(&self, size: u32, scale: u32) -> u32 {
        let (min, max) = match self.kind {
            Kind::Fixed => (self.size, self.size),
            _ => (self.min, self.max),
        };
        let target = size * scale;
        let (min, max) = (min * self.scale, max * self.scale);
        if target < min {
            min - target
        } else {
            target.saturating_sub(max)
        }
    }
    fn matches(&self, size: u32, scale: u32) -> bool {
        self.scale == scale && self.distance(size, scale) == 0
    }
}
#[derive(Default)]
struct Theme {
    directories: Vec<Directory>,
    parents: Vec<String>,
}
pub struct Resolver {
    roots: Vec<PathBuf>,
    pixmaps: Vec<PathBuf>,
    selected: String,
    themes: HashMap<String, Theme>,
}

impl Resolver {
    pub fn from_environment() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute());
        let data_home = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| home.as_ref().map(|p| p.join(".local/share")));
        let dirs = std::env::var_os("XDG_DATA_DIRS")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
        let data_dirs: Vec<_> = std::env::split_paths(&dirs)
            .filter(|p| p.is_absolute())
            .take(32)
            .collect();
        let mut roots = Vec::new();
        if let Some(home) = home {
            roots.push(home.join(".icons"));
        }
        if let Some(home) = data_home {
            roots.push(home.join("icons"));
        }
        roots.extend(data_dirs.iter().map(|p| p.join("icons")));
        let selected = std::env::var("TAROT_ICON_THEME")
            .ok()
            .filter(|s| valid_theme(s))
            .unwrap_or_else(|| "Adwaita".into());
        Self {
            roots,
            pixmaps: data_dirs.iter().map(|p| p.join("pixmaps")).collect(),
            selected,
            themes: HashMap::new(),
        }
    }
    pub fn lookup(&mut self, name: &str, (size, scale): (u32, u32)) -> Option<PathBuf> {
        if name.len() > 4096 || name.is_empty() {
            return None;
        }
        let absolute = Path::new(name);
        if absolute.is_absolute() {
            return supported(absolute).then(|| absolute.to_owned());
        }
        if !valid_theme(name) {
            return None;
        }
        let mut chain = Vec::new();
        self.chain(self.selected.clone(), &mut HashSet::new(), &mut chain);
        if !chain.iter().any(|s| s == "hicolor") {
            // Reserve a slot even when a malicious inheritance chain hits its cap.
            self.load_theme("hicolor");
            chain.push("hicolor".into());
        }
        for theme in chain {
            let metadata = &self.themes[&theme];
            // An exact match anywhere in this theme precedes a closer root.
            for directory in metadata
                .directories
                .iter()
                .filter(|d| d.matches(size, scale))
            {
                if let Some(path) = self.in_directory(&theme, directory, name) {
                    return Some(path);
                }
            }
            let mut closest: Vec<_> = metadata.directories.iter().collect();
            closest.sort_by_key(|d| d.distance(size, scale));
            for directory in closest {
                if let Some(path) = self.in_directory(&theme, directory, name) {
                    return Some(path);
                }
            }
        }
        for root in self.roots.iter().chain(self.pixmaps.iter()) {
            for extension in ["png", "svg"] {
                let path = root.join(format!("{name}.{extension}"));
                if path.is_file() {
                    return Some(path);
                }
            }
        }
        None
    }
    fn in_directory(&self, theme: &str, directory: &Directory, name: &str) -> Option<PathBuf> {
        for root in &self.roots {
            for extension in ["png", "svg"] {
                let path = root
                    .join(theme)
                    .join(&directory.name)
                    .join(format!("{name}.{extension}"));
                if path.is_file() {
                    return Some(path);
                }
            }
        }
        None
    }
    fn chain(&mut self, name: String, seen: &mut HashSet<String>, chain: &mut Vec<String>) {
        if chain.len() >= 31 || !valid_theme(&name) || !seen.insert(name.clone()) {
            return;
        }
        self.load_theme(&name);
        let parents = self.themes[&name].parents.clone();
        chain.push(name);
        for parent in parents {
            self.chain(parent, seen, chain);
        }
    }
    fn load_theme(&mut self, name: &str) {
        if !self.themes.contains_key(name) {
            let parsed = self
                .roots
                .iter()
                .find_map(|root| super::read(&root.join(name).join("index.theme"), 256 * 1024))
                .and_then(|b| String::from_utf8(b).ok())
                .map(|s| parse(&s))
                .unwrap_or_default();
            self.themes.insert(name.to_owned(), parsed);
        }
    }
}
fn supported(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("png" | "svg")
    )
}
fn valid_theme(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name != "."
        && name != ".."
        && !name
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '/' | '\\' | ',' | '\0'))
}
fn valid_directory(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 512
        && Path::new(name)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}
fn parse(text: &str) -> Theme {
    let mut sections: HashMap<&str, HashMap<&str, &str>> = HashMap::new();
    let mut section = "";
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = name;
        } else if let Some((key, value)) = line.split_once('=') {
            sections
                .entry(section)
                .or_default()
                .insert(key.trim(), value.trim());
        }
    }
    let Some(index) = sections.get("Icon Theme") else {
        return Theme::default();
    };
    let parents = index
        .get("Inherits")
        .into_iter()
        .flat_map(|s| s.split(','))
        .map(str::trim)
        .filter(|s| valid_theme(s))
        .take(32)
        .map(str::to_owned)
        .collect();
    let mut directories = Vec::new();
    let mut seen = HashSet::new();
    for name in ["Directories", "ScaledDirectories"]
        .iter()
        .flat_map(|k| index.get(k))
        .flat_map(|s| s.split(','))
        .map(str::trim)
        .take(1024)
    {
        if !valid_directory(name) || !seen.insert(name) {
            continue;
        }
        let Some(section) = sections.get(name) else {
            continue;
        };
        let number = |key, default| {
            section
                .get(key)
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(default)
                .min(16_384)
        };
        let size = number("Size", 0);
        if size == 0 {
            continue;
        }
        let scale = number("Scale", 1).clamp(1, 16);
        let kind = match section.get("Type").copied().unwrap_or("Threshold") {
            "Fixed" => Kind::Fixed,
            "Scalable" => Kind::Scalable,
            "Threshold" => Kind::Threshold,
            _ => continue,
        };
        let (min, max) = match kind {
            Kind::Fixed => (size, size),
            Kind::Scalable => (number("MinSize", size), number("MaxSize", size)),
            Kind::Threshold => {
                let threshold = number("Threshold", 2);
                (
                    size.saturating_sub(threshold),
                    size.saturating_add(threshold),
                )
            }
        };
        if min > max {
            continue;
        }
        directories.push(Directory {
            name: name.into(),
            size,
            scale,
            min,
            max,
            kind,
        });
    }
    Theme {
        directories,
        parents,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn precedence_scale_inheritance_and_traversal() {
        let temp = tempfile::tempdir().unwrap();
        let a = temp.path().join("user");
        let b = temp.path().join("system");
        for root in [&a, &b] {
            std::fs::create_dir_all(root.join("Test")).unwrap();
        }
        std::fs::write(a.join("Test/index.theme"),"[Icon Theme]\nInherits=Parent\nDirectories=small,big,scale\n[small]\nSize=32\nType=Fixed\n[big]\nSize=64\nType=Fixed\n[scale]\nSize=32\nScale=2\nType=Fixed\n").unwrap();
        let put = |root: &Path, path: &str| {
            let p = root.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, b"fixture").unwrap();
            p
        };
        let small = put(&a, "Test/small/app.png");
        let scaled = put(&b, "Test/scale/app.svg");
        let big = put(&b, "Test/big/app.png");
        std::fs::create_dir_all(b.join("Parent")).unwrap();
        std::fs::write(
            b.join("Parent/index.theme"),
            "[Icon Theme]\nInherits=Test\nDirectories=apps\n[apps]\nSize=58\n",
        )
        .unwrap();
        let parent = put(&b, "Parent/apps/parent.png");
        let mut r = Resolver {
            roots: vec![a, b],
            pixmaps: vec![],
            selected: "Test".into(),
            themes: HashMap::new(),
        };
        assert_eq!(r.lookup("app", (32, 1)), Some(small));
        assert_eq!(r.lookup("app", (32, 2)), Some(scaled));
        assert_eq!(r.lookup("app", (58, 1)), Some(big));
        assert_eq!(r.lookup("parent", (58, 1)), Some(parent));
        assert!(r.lookup("../secret", (58, 1)).is_none());
        assert!(r.lookup("/tmp/icon.xpm", (58, 1)).is_none());
        assert!(!valid_directory("../apps"));
        assert!(!valid_directory("/apps"));
    }
}
