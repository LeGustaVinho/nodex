use super::*;
use nodex_core::units::BuildUnit;
use nodex_ir::Language;

pub(super) struct Application {
    pub node: ArchitectureNode,
    root: String,
    /// The entry directory or file used to assign code, within the build unit.
    scope: String,
    entries: BTreeSet<String>,
}

impl Application {
    pub fn root(&self) -> &str {
        &self.root
    }

    pub fn legacy_id(&self) -> Option<&str> {
        let root = self.node.id.strip_prefix("app:")?.split_once("::")?.0;
        Some(&self.node.id[.."app:".len() + root.len()])
    }
}

#[derive(Clone)]
struct Candidate {
    name: String,
    technology: &'static str,
    scope: String,
    evidence: Vec<ArchitectureEvidence>,
    entries: BTreeSet<String>,
}

pub(super) fn unit_for<'a>(input: &'a UmlInput<'_>, file: &str) -> Option<&'a BuildUnit> {
    input
        .units
        .iter()
        .filter(|u| within(file, &u.root))
        .max_by_key(|u| u.root.len())
}

pub(super) fn infer<'a>(input: &'a UmlInput<'_>, index: &Index<'a>) -> Vec<Application> {
    let mut roots: BTreeMap<&str, BTreeMap<(&str, String), Candidate>> = BTreeMap::new();
    let mut add = |symbol: &Symbol,
                   site: &'a Span,
                   technology: &'static str,
                   reason: &str,
                   binary: Option<&str>| {
        let file = &site.file;
        let unit = input
            .units
            .iter()
            .filter(|u| u.language == symbol.language && within(file, &u.root))
            .max_by_key(|u| u.root.len());
        let dir = dirname(file);
        let fallback = if dir == "src" {
            ""
        } else {
            dir.strip_suffix("/src").unwrap_or(dir)
        };
        let root = unit.map_or(fallback, |u| u.root.as_str());
        let scope = match symbol.language {
            Language::Csharp => root.to_owned(),
            Language::Typescript => module_path(file, root),
            Language::Rust
                if leaf(file) == "main.rs" && dir != root && relative(dir, root) != "src" =>
            {
                dir.to_owned()
            }
            Language::Rust => file.clone(),
        };
        let apps = roots.entry(root).or_default();
        let key = if symbol.language == Language::Csharp {
            root
        } else {
            file
        };
        let app = apps
            .entry((technology, key.into()))
            .or_insert_with(|| Candidate {
                name: unit
                    .map(|u| u.name.as_str())
                    .unwrap_or(if root.is_empty() {
                        input.project
                    } else {
                        leaf(root)
                    })
                    .into(),
                technology,
                scope,
                evidence: unit
                    .into_iter()
                    .map(|u| ArchitectureEvidence::file("Build unit", &u.manifest))
                    .collect(),
                entries: BTreeSet::new(),
            });
        if let Some(binary) = binary.filter(|_| relative(file, root) != "src/main.rs") {
            app.name = binary.into();
        }
        app.entries.insert(file.clone());
        push_evidence(
            &mut app.evidence,
            ArchitectureEvidence::at(reason, Some(site), Some(&symbol.id)),
        );
    };

    let mut checked = BTreeSet::new();
    for symbol in index
        .graph
        .symbols
        .iter()
        .filter(|s| diagram_symbol(s, index))
    {
        for site in &symbol.origins {
            if symbol.language == Language::Rust {
                if let Some(name) = rust_binary(index, input, symbol, site) {
                    add(
                        symbol,
                        site,
                        "Rust",
                        "Cargo binary entry (main)",
                        Some(name),
                    );
                }
                continue;
            }
            if matches!(
                entry(index, &symbol.id),
                Some(EntryKind::Main | EntryKind::ServerStart)
            ) {
                let technology = if symbol.language == Language::Csharp {
                    ".NET"
                } else {
                    "Node.js"
                };
                let reason = &index.info[symbol.id.as_str()].entry.as_ref().unwrap().label;
                add(symbol, site, technology, reason, None);
            }
            if symbol.language == Language::Typescript
                && checked.insert(site.file.as_str())
                && let Some(framework) = browser_framework(input, &site.file)
            {
                add(
                    symbol,
                    site,
                    framework,
                    "Browser application bootstrap",
                    None,
                );
            }
        }
    }

    let mut javascript = BTreeSet::new();
    let mut imports = BTreeMap::<&str, BTreeSet<&str>>::new();
    if input.units.iter().any(|u| !u.node_entries.is_empty()) {
        javascript.extend(
            index
                .graph
                .symbols
                .iter()
                .filter(|s| s.language == Language::Typescript && diagram_symbol(s, index))
                .flat_map(|s| &s.origins)
                .map(|site| site.file.as_str()),
        );
        for (from, to) in exact_file_dependencies_of(index, |kind| {
            matches!(kind, StructEdgeKind::Imports | StructEdgeKind::Calls)
        }) {
            imports.entry(from).or_default().insert(to);
        }
    }
    roots
        .into_iter()
        .flat_map(|(root, mut apps)| {
            if let Some(unit) = input
                .units
                .iter()
                .find(|u| u.root == root && !u.node_entries.is_empty())
            {
                group_launched_listeners(unit, &javascript, &imports, &mut apps);
            }
            let split = apps.len() > 1;
            let mut counts = BTreeMap::new();
            for app in apps.values() {
                *counts.entry(app.scope.clone()).or_insert(0) += 1;
            }
            apps.into_iter().map(move |((_, key), app)| {
                let scope = if counts[&app.scope] > 1 {
                    key.clone()
                } else {
                    app.scope
                };
                let exact_scope = split
                    || (app.technology == "Rust"
                        && relative(&scope, root) != "src/main.rs"
                        && relative(&scope, root) != "main.rs");
                let id = format!("app:{root}::{}:{key}", app.technology);
                let name = if split && app.technology != "Rust" {
                    let location = if scope == root { "" } else { leaf(&scope) };
                    [app.name.as_str(), app.technology, location]
                        .into_iter()
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" · ")
                } else {
                    app.name
                };
                let mut node = ArchitectureNode::new(
                    id,
                    name,
                    ArchitectureKind::Application,
                    Provenance::Inferred,
                );
                node.path = Some(scope.clone());
                node.technology = Some(app.technology.into());
                node.evidence = app.evidence;
                Application {
                    node,
                    root: root.into(),
                    scope: if exact_scope { scope } else { root.into() },
                    entries: app.entries,
                }
            })
        })
        .collect()
}

fn group_launched_listeners(
    unit: &BuildUnit,
    sources: &BTreeSet<&str>,
    imports: &BTreeMap<&str, BTreeSet<&str>>,
    apps: &mut BTreeMap<(&str, String), Candidate>,
) {
    let launches: BTreeSet<_> = unit
        .node_entries
        .iter()
        .filter_map(|script| {
            let (stem, extension) = script.rsplit_once('.')?;
            let compiled = relative(stem, &unit.root);
            let source = compiled
                .strip_prefix("dist/")
                .or_else(|| compiled.strip_prefix("build/"));
            let rebased = source.map(|path| {
                if unit.root.is_empty() {
                    format!("src/{path}")
                } else {
                    format!("{}/src/{path}", unit.root)
                }
            });
            let extensions: &[_] = match extension {
                "mjs" => &["mts", "mjs"],
                "cjs" => &["cts", "cjs"],
                _ => &["ts", "tsx", "js"],
            };
            sources.get(script.as_str()).copied().or_else(|| {
                rebased
                    .as_deref()
                    .into_iter()
                    .chain([stem])
                    .flat_map(|stem| {
                        extensions
                            .iter()
                            .map(move |extension| format!("{stem}.{extension}"))
                    })
                    .find_map(|file| sources.get(file.as_str()).copied())
            })
        })
        .collect();
    let mut launched_by = BTreeMap::<&str, Vec<&str>>::new();
    for &launch in &launches {
        let mut visit = vec![launch];
        let mut seen = BTreeSet::new();
        while let Some(file) = visit.pop() {
            if !seen.insert(file) {
                continue;
            }
            launched_by.entry(file).or_default().push(launch);
            visit.extend(
                imports
                    .get(file)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|file| within(file, &unit.root) && !launches.contains(file)),
            );
        }
    }
    let mut listeners = Vec::new();
    apps.retain(|(technology, file), app| {
        if *technology == "Node.js"
            && !launches.contains(file.as_str())
            && let Some(anchors) = launched_by.get(file.as_str())
        {
            listeners.push((anchors, app.clone()));
            return false;
        }
        true
    });
    for (anchors, listener) in listeners {
        for &entry in anchors {
            let app = apps
                .entry(("Node.js", entry.into()))
                .or_insert_with(|| Candidate {
                    scope: module_path(entry, &unit.root),
                    entries: BTreeSet::from([entry.into()]),
                    evidence: vec![ArchitectureEvidence::file(
                        "Node.js launch script",
                        &unit.manifest,
                    )],
                    ..listener.clone()
                });
            app.entries.extend(listener.entries.iter().cloned());
            for evidence in &listener.evidence {
                push_evidence(&mut app.evidence, evidence.clone());
            }
        }
    }
}

fn rust_binary<'a>(
    index: &Index<'_>,
    input: &'a UmlInput<'_>,
    symbol: &Symbol,
    site: &Span,
) -> Option<&'a str> {
    if symbol.kind != SymbolKind::Function || symbol.name != "main" || !symbol.has_flow {
        return None;
    }
    let module = index.get(symbol.parent.as_deref()?)?;
    if module.kind != SymbolKind::Module
        || module.flavors.iter().any(|f| f == "inline-module")
        || !module.origins.iter().any(|origin| {
            origin.file == site.file && origin.start_byte == 0 && origin.end_byte >= site.end_byte
        })
    {
        return None;
    }
    let unit = input
        .units
        .iter()
        .filter(|u| u.language == Language::Rust && within(&site.file, &u.root))
        .max_by_key(|u| u.root.len());
    match unit {
        Some(unit) => unit.rust_binaries.get(&site.file).map(String::as_str),
        None if matches!(site.file.as_str(), "src/main.rs" | "main.rs") => Some(input.project),
        None => None,
    }
}

fn browser_framework(input: &UmlInput<'_>, file: &str) -> Option<&'static str> {
    if !matches!(
        leaf(file).split('.').next()?,
        "main" | "index" | "bootstrap"
    ) {
        return None;
    }
    let source = input.flows.source(file)?;
    if source.contains("react-dom/client")
        && source.contains("createRoot(")
        && source.contains(".render(")
    {
        Some("React")
    } else if source.contains("createApp(") && source.contains(".mount(") {
        Some("Vue")
    } else if source.contains("@angular/platform-browser")
        && source.contains("bootstrapApplication(")
    {
        Some("Angular")
    } else {
        None
    }
}

pub(super) fn apply_declarations(
    mut apps: Vec<Application>,
    config: &ArchitectureConfig,
) -> Vec<Application> {
    let scopes = scope_aliases(&apps)
        .into_iter()
        .map(|(alias, id)| (alias, id.to_owned()))
        .collect::<BTreeMap<_, _>>();
    let mut declarations: Vec<_> = config.applications.iter().collect();
    declarations.sort_by_key(|app| app.root.len());
    for app in declarations {
        let id = format!("app:{}", app.root);
        let mut node = ArchitectureNode::new(
            id.clone(),
            app.name.clone(),
            ArchitectureKind::Application,
            Provenance::Declared,
        );
        node.path = Some(app.root.clone());
        node.technology = Some(app.technology.clone()).filter(|s| !s.is_empty());
        let evidence = ArchitectureEvidence::at(
            "Application declared in local architecture settings",
            None,
            None,
        );
        node.evidence.push(evidence.clone());
        let target = scopes.get(&id).unwrap_or(&id);
        if let Some(inferred) = apps.iter_mut().find(|a| &a.node.id == target) {
            inferred.node.name = node.name;
            if inferred.legacy_id().is_none() || node.technology.is_some() {
                inferred.node.technology = node.technology;
            }
            inferred.node.provenance = Provenance::Declared;
            push_evidence(&mut inferred.node.evidence, evidence);
        } else if apps.iter().any(|a| a.legacy_id() == Some(&id)) {
            // A previously shared declaration is a label for the split boundary.
            for inferred in apps
                .iter_mut()
                .filter(|a| a.legacy_id() == Some(&id) && a.node.provenance != Provenance::Declared)
            {
                inferred.node.name = format!("{} · {}", app.name, inferred.node.name);
                inferred.node.provenance = Provenance::Declared;
                push_evidence(&mut inferred.node.evidence, evidence.clone());
            }
        } else {
            apps.push(Application {
                node,
                root: app.root.clone(),
                scope: app.root.clone(),
                entries: BTreeSet::new(),
            });
        }
    }
    apps
}

/// Resolve former IDs and project-relative paths when they identify one executable.
pub(super) fn scope_aliases(apps: &[Application]) -> BTreeMap<String, &str> {
    let mut aliases = BTreeMap::new();
    let ids: BTreeSet<_> = apps.iter().map(|app| app.node.id.as_str()).collect();
    for app in apps {
        if let Some(legacy) = app.legacy_id() {
            let mut paths = BTreeSet::from([legacy.to_owned()]);
            if let Some(path) = &app.node.path {
                paths.insert(format!("app:{path}"));
            }
            for entry in &app.entries {
                paths.insert(format!("app:{entry}"));
                if app.node.id.starts_with(&format!("{legacy}::Node.js:")) {
                    paths.insert(format!("{legacy}::Node.js:{entry}"));
                }
            }
            for alias in paths {
                if ids.contains(alias.as_str()) {
                    continue;
                }
                aliases
                    .entry(alias)
                    .and_modify(|value| *value = None)
                    .or_insert(Some(app.node.id.as_str()));
            }
        }
    }
    aliases
        .into_iter()
        .filter_map(|(path, id)| Some((path, id?)))
        .collect()
}

pub(super) fn membership<'a>(
    index: &Index<'a>,
    input: &UmlInput<'_>,
    apps: &'a [Application],
) -> BTreeMap<&'a str, BTreeSet<&'a str>> {
    let mut files = BTreeMap::new();
    for symbol in index
        .graph
        .symbols
        .iter()
        .filter(|s| diagram_symbol(s, index))
    {
        for site in &symbol.origins {
            files.entry(site.file.as_str()).or_insert_with(|| {
                let unit = unit_for(input, &site.file);
                let covered = apps.iter().filter(|app| {
                    (within(&site.file, &app.scope) || app.entries.contains(&site.file))
                        && same_unit(unit, app)
                });
                let entries: BTreeSet<_> = covered
                    .clone()
                    .filter(|a| a.entries.contains(&site.file))
                    .map(|a| a.node.id.as_str())
                    .collect();
                if !entries.is_empty() {
                    return entries;
                }
                let best = covered.clone().map(|a| a.scope.len()).max();
                let mut candidates = covered.filter(|a| Some(a.scope.len()) == best);
                let one = candidates.next();
                one.filter(|_| candidates.next().is_none())
                    .into_iter()
                    .map(|a| a.node.id.as_str())
                    .collect()
            });
        }
    }
    let mut dependencies = BTreeMap::<&str, BTreeSet<&str>>::new();
    for (from, to) in exact_file_dependencies(index) {
        dependencies.entry(from).or_default().insert(to);
    }
    let mut reached = BTreeMap::<&str, BTreeSet<&str>>::new();
    for app in apps.iter().filter(|app| app.scope != app.root()) {
        let id = app.node.id.as_str();
        let mut seen = BTreeSet::new();
        let mut visit: Vec<_> = files
            .iter()
            .filter(|(_, owners)| owners.contains(id))
            .map(|(&file, _)| file)
            .collect();
        while let Some(file) = visit.pop() {
            if !seen.insert(file) {
                continue;
            }
            for &target in dependencies.get(file).into_iter().flatten() {
                if files.get(target).is_some_and(BTreeSet::is_empty)
                    && within(target, app.root())
                    && same_unit(unit_for(input, target), app)
                {
                    reached.entry(target).or_default().insert(id);
                    visit.push(target);
                }
            }
        }
    }
    for (file, owners) in reached {
        files.insert(file, owners);
    }
    files
}

fn same_unit(unit: Option<&BuildUnit>, app: &Application) -> bool {
    unit.is_none_or(|unit| unit.root == app.root() || !within(&unit.root, app.root()))
}

pub(super) fn exact_file_dependencies<'a>(
    index: &Index<'a>,
) -> impl Iterator<Item = (&'a str, &'a str)> {
    exact_file_dependencies_of(index, static_relation)
}

fn exact_file_dependencies_of<'a>(
    index: &Index<'a>,
    kind: impl Fn(StructEdgeKind) -> bool,
) -> impl Iterator<Item = (&'a str, &'a str)> {
    let file = move |id: &str| {
        let symbol = index.get(id)?;
        if symbol.external || index.is_test(id) {
            return None;
        }
        let first = symbol.origins.first()?.file.as_str();
        symbol
            .origins
            .iter()
            .all(|o| o.file == first)
            .then_some(first)
    };
    index
        .relations()
        .filter(move |edge| {
            kind(edge.kind) && edge.confidence.unwrap_or(Confidence::Exact) == Confidence::Exact
        })
        .flat_map(move |edge| {
            let target = (!index.is_test(&edge.from))
                .then(|| file(&edge.to))
                .flatten();
            let unlocated = edge.site.is_none().then(|| file(&edge.from)).flatten();
            edge.sites()
                .map(|site| site.file.as_str())
                .chain(unlocated)
                .filter_map(move |from| target.filter(|to| from != *to).map(|to| (from, to)))
        })
}
