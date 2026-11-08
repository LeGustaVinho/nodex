//! The on-disk lowering cache: a reopen reads unchanged files back, and whatever happens to
//! the cache, the project comes out the same as without it.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use nodex_core::{Project, cache};
use nodex_ir::FileIr;
use nodex_lang_api::LanguageFrontend;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn frontends() -> Vec<Arc<dyn LanguageFrontend>> {
    vec![
        Arc::new(nodex_lang_csharp::CSharpFrontend),
        Arc::new(nodex_lang_typescript::TypeScriptFrontend),
        Arc::new(nodex_lang_rust::RustFrontend),
    ]
}

/// A fresh folder for one test, with the C#, TypeScript and Rust samples in it (sources only)
/// and a cache folder next to it. Files are dated an hour back: the cache leaves out files
/// changed in the last seconds.
struct Scratch {
    dir: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("nodex-cache-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for sample in ["csharp/Sample", "ts/Sample", "rust/Sample"] {
            copy_sources(&fixtures().join(sample), &dir.join("project").join(sample.replace('/', "-")));
        }
        Self { dir }
    }

    fn root(&self) -> PathBuf {
        self.dir.join("project")
    }

    fn cache(&self) -> PathBuf {
        self.dir.join("cache")
    }

    fn open(&self) -> Project {
        Project::open_cached(&self.root(), frontends(), Some(&self.cache()), &|_| {}).expect("opens with the cache")
    }

    /// The project opened without a cache: what every cached open must equal.
    fn reference(&self) -> Project {
        Project::open(&self.root(), frontends()).expect("opens")
    }

    fn bundle(&self, project: &Project) -> PathBuf {
        cache::bundle(&self.cache(), project.root())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn copy_sources(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        let target = to.join(&name);
        if entry.file_type().unwrap().is_dir() {
            if !nodex_lang_api::is_ignored_dir(&name) && name != ".nodex" {
                copy_sources(&entry.path(), &target);
            }
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
            set_age(&target, Duration::from_secs(3600));
        }
    }
}

fn set_age(path: &Path, age: Duration) {
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - age).unwrap();
}

/// Every lowered file, the structure graph and the knowledge, for comparing two opens.
fn contents(project: &mut Project) -> (Vec<(String, FileIr)>, nodex_ir::StructureGraph, nodex_ir::knowledge::ProjectKnowledge) {
    let files = project.files().map(|(rel, ir)| (rel.to_owned(), ir.clone())).collect();
    (files, (*project.structure()).clone(), (*project.knowledge()).clone())
}

#[test]
fn a_reopen_reads_every_unchanged_file_back() {
    let scratch = Scratch::new("reopen");
    let expected = contents(&mut scratch.reference());
    assert!(expected.0.len() >= 6, "the samples have files of each language");

    let mut cold = scratch.open();
    assert_eq!(cold.files_from_cache(), 0);
    assert_eq!(contents(&mut cold), expected);
    assert!(scratch.bundle(&cold).is_file(), "the first open writes the cache");

    let mut warm = scratch.open();
    assert_eq!(warm.files_from_cache(), expected.0.len());
    assert_eq!(contents(&mut warm), expected);
}

#[test]
fn changed_files_are_lowered_again_and_deleted_ones_pruned() {
    let scratch = Scratch::new("changes");
    let first = scratch.open();
    let total = first.files().count();
    let changed = first.files().map(|(rel, _)| rel.to_owned()).find(|rel| rel.ends_with(".cs")).unwrap();
    let deleted = first.files().map(|(rel, _)| rel.to_owned()).find(|rel| rel.ends_with(".ts")).unwrap();

    let path = first.absolute(&changed).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{text}\nnamespace Added {{ public class AddedLater {{ void Run() {{ }} }} }}\n")).unwrap();
    set_age(&path, Duration::from_secs(1800));
    std::fs::remove_file(first.absolute(&deleted).unwrap()).unwrap();

    let mut second = scratch.open();
    assert_eq!(second.files().count(), total - 1);
    assert_eq!(second.files_from_cache(), total - 2, "all but the changed file come from the cache");
    assert!(second.file_ir(&changed).unwrap().symbols.iter().any(|s| s.name == "AddedLater"));
    assert_eq!(contents(&mut second), contents(&mut scratch.reference()));
    let bundle = cache::LowerCache::load(&scratch.cache(), second.root());
    assert_eq!(bundle.len(), total - 1, "the deleted file is pruned, the changed one kept");

    let third = scratch.open();
    assert_eq!(third.files_from_cache(), total - 1);
}

#[test]
fn files_changed_just_now_are_not_cached() {
    let scratch = Scratch::new("racy");
    let first = scratch.open();
    let total = first.files().count();
    let (fresh, _) = first.files().next().unwrap();
    let path = first.absolute(fresh).unwrap();
    std::fs::File::options().write(true).open(&path).unwrap().set_modified(SystemTime::now()).unwrap();

    // The touched file is lowered, and stays out of the cache while its time is that recent.
    assert_eq!(scratch.open().files_from_cache(), total - 1);
    assert_eq!(scratch.open().files_from_cache(), total - 1);
}

#[test]
fn a_damaged_cache_never_fails_the_open() {
    let scratch = Scratch::new("damaged");
    let expected = contents(&mut scratch.reference());
    let total = expected.0.len();
    let bundle = scratch.bundle(&scratch.open());
    let good = std::fs::read(&bundle).unwrap();

    let mut flipped = good.clone();
    for i in (good.len() / 3..good.len()).step_by(good.len() / 7) {
        flipped[i] ^= 0xa5;
    }
    let damages: [(&str, Vec<u8>); 4] = [
        ("flipped bytes", flipped),
        ("truncated", good[..good.len() / 2].to_vec()),
        ("garbage", b"not a cache at all".to_vec()),
        ("empty", Vec::new()),
    ];
    for (what, bytes) in damages {
        std::fs::write(&bundle, bytes).unwrap();
        let mut project = scratch.open();
        assert!(project.files_from_cache() < total, "{what}: damaged entries are not used");
        assert_eq!(contents(&mut project), expected, "{what}: the project is the same as without a cache");
        assert_eq!(scratch.open().files_from_cache(), total, "{what}: the open repaired the cache");
    }

    // A folder in the bundle's place cannot be read or replaced: the open still works.
    std::fs::remove_file(&bundle).unwrap();
    std::fs::create_dir_all(bundle.join("blocked")).unwrap();
    let mut project = scratch.open();
    assert_eq!(project.files_from_cache(), 0);
    assert_eq!(contents(&mut project), expected);
}

#[test]
fn clearing_makes_the_next_open_cold() {
    let scratch = Scratch::new("clear");
    let total = scratch.open().files().count();
    assert_eq!(scratch.open().files_from_cache(), total);
    cache::clear(&scratch.cache(), &scratch.root()).unwrap();
    assert_eq!(scratch.open().files_from_cache(), 0);
}
