use super::*;
use nodex_core::units::BuildUnit;
use nodex_ir::Language;

pub(super) struct Application {
    pub node: ArchitectureNode,
    /// The entry directory or file used to assign code, within the build unit.
    scope: String,
    entries: BTreeSet<String>,
}

impl Application {
    pub fn root(&self) -> &str {
        self.node.path.as_deref().expect("application has a root")
    }

    pub fn legacy_id(&self) -> Option<&str> {
        let root = self.node.id.strip_prefix("app:")?.split_once("::")?.0;
        Some(&self.node.id[.."app:".len() + root.len()])
    }
}

pub(super) fn unit_for<'a>(input: &'a UmlInput<'_>, file: &str) -> Option<&'a BuildUnit> {
    input
        .units
        .iter()
        .filter(|u| within(file, &u.root))
        .max_by_key(|u| u.root.len())
}

pub(super) fn infer<'a>(input: &'a UmlInput<'_>, index: &Index<'a>) -> Vec<Application> {
    struct Candidate {
        name: String,
        technology: &'static str,
        scope: String,
        evidence: Vec<ArchitectureEvidence>,
        entries: BTreeSet<String>,
    }
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

    roots
        .into_iter()
        .flat_map(|(root, apps)| {
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
                let id = if split {
                    format!("app:{root}::{}:{key}", app.technology)
                } else {
                    format!("app:{root}")
                };
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
                node.path = Some(root.into());
                node.technology = Some(app.technology.into());
                node.evidence = app.evidence;
                Application {
                    node,
                    scope: if exact_scope { scope } else { root.into() },
                    entries: app.entries,
                }
            })
        })
        .collect()
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
    for app in &config.applications {
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
        if let Some(inferred) = apps.iter_mut().find(|a| a.node.id == id) {
            node.evidence.append(&mut inferred.node.evidence);
            inferred.node = node;
        } else if apps.iter().any(|a| a.legacy_id() == Some(&id)) {
            // A previously shared declaration is a label for the split boundary.
            for inferred in apps.iter_mut().filter(|a| a.legacy_id() == Some(&id)) {
                inferred.node.name = format!("{} · {}", app.name, inferred.node.name);
                inferred.node.provenance = Provenance::Declared;
                push_evidence(&mut inferred.node.evidence, evidence.clone());
            }
        } else {
            apps.push(Application {
                node,
                scope: app.root.clone(),
                entries: BTreeSet::new(),
            });
        }
    }
    apps
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
                let covered = apps
                    .iter()
                    .filter(|app| within(&site.file, &app.scope) && same_unit(unit, app));
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
    index
        .graph
        .symbols
        .iter()
        .filter(|s| diagram_symbol(s, index))
        .filter_map(|s| {
            let owners: BTreeSet<_> = s
                .origins
                .iter()
                .filter_map(|o| files.get(o.file.as_str()))
                .flatten()
                .copied()
                .collect();
            (!owners.is_empty()).then_some((s.id.as_str(), owners))
        })
        .collect()
}

fn same_unit(unit: Option<&BuildUnit>, app: &Application) -> bool {
    unit.is_none_or(|unit| unit.root == app.root() || !within(&unit.root, app.root()))
}

pub(super) fn exact_file_dependencies<'a>(
    index: &Index<'a>,
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
        .filter(|edge| {
            static_relation(edge.kind)
                && edge.confidence.unwrap_or(Confidence::Exact) == Confidence::Exact
        })
        .filter_map(move |edge| {
            let from = edge
                .site
                .as_ref()
                .map(|s| s.file.as_str())
                .or_else(|| file(&edge.from))?;
            let to = file(&edge.to)?;
            (from != to && !index.is_test(&edge.from)).then_some((from, to))
        })
}
