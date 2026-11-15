use super::applications::{
    Application, exact_file_dependencies, membership, scope_aliases, unit_for,
};
use super::*;

const SYSTEM: &str = "system";

#[derive(Clone)]
struct Link {
    from: String,
    to: String,
    from_symbol: Option<String>,
    to_symbol: Option<String>,
    kind: &'static str,
    label: String,
    confidence: Confidence,
    provenance: Provenance,
    evidence: ArchitectureEvidence,
}

fn declared(
    out: &mut BTreeMap<String, ArchitectureNode>,
    items: &[DeclaredElement],
    kind: ArchitectureKind,
    prefix: &str,
) {
    for item in items {
        let id = format!("{prefix}:{}", item.id);
        let mut node =
            ArchitectureNode::new(id.clone(), item.name.clone(), kind, Provenance::Declared);
        node.technology = Some(item.technology.clone()).filter(|s| !s.is_empty());
        node.evidence.push(ArchitectureEvidence::at(
            "Declared in local architecture settings",
            None,
            None,
        ));
        out.insert(id, node);
    }
}

pub(super) fn build(
    index: &Index<'_>,
    input: &UmlInput<'_>,
    request: &ArchitectureRequest,
    config: &ArchitectureConfig,
    apps: Vec<Application>,
) -> Result<ArchitectureDiagram, UmlError> {
    let name = if config.system_name.is_empty() {
        input.project
    } else {
        &config.system_name
    };
    let mut result = ArchitectureDiagram::new(format!("C4 · {name}"));
    let mut elements = BTreeMap::new();
    let system = ArchitectureNode::new(
        SYSTEM.into(),
        name.into(),
        ArchitectureKind::System,
        if config.system_name.is_empty() {
            Provenance::Inferred
        } else {
            Provenance::Declared
        },
    );
    elements.insert(SYSTEM.into(), system.clone());
    result.applications = apps.iter().map(|a| a.node.clone()).collect();
    let aliases = scope_aliases(&apps);
    let excluded: BTreeSet<_> = config
        .excluded_apps
        .iter()
        .map(|id| aliases.get(id).copied().unwrap_or(id))
        .collect();
    let active: BTreeSet<&str> = apps
        .iter()
        .filter(|a| {
            !excluded.contains(a.node.id.as_str())
                && a.legacy_id().is_none_or(|id| !excluded.contains(id))
        })
        .map(|a| a.node.id.as_str())
        .collect();
    for app in result
        .applications
        .iter()
        .filter(|n| active.contains(n.id.as_str()))
    {
        elements.insert(app.id.clone(), app.clone());
    }
    declared(
        &mut elements,
        &config.actors,
        ArchitectureKind::Actor,
        "actor",
    );
    declared(
        &mut elements,
        &config.external_systems,
        ArchitectureKind::External,
        "external",
    );
    declared(
        &mut elements,
        &config.stores,
        ArchitectureKind::Store,
        "store",
    );

    let files = membership(index, input, &apps);
    let executable_roots: BTreeSet<_> = apps.iter().map(Application::root).collect();
    let mut explicit_paths = BTreeMap::<&str, BTreeSet<&str>>::new();
    for item in &config.libraries {
        let application = aliases
            .get(&item.application)
            .copied()
            .unwrap_or(&item.application);
        if active.contains(application) {
            explicit_paths
                .entry(application)
                .or_default()
                .insert(&item.path);
        } else if apps
            .iter()
            .any(|a| a.legacy_id() == Some(&item.application))
        {
            push_evidence(
                &mut result.pending,
                ArchitectureEvidence::file(
                    "Select an application for this library after the build unit was split",
                    &item.path,
                ),
            );
        }
    }
    let library_path = |file: &str, app: &str| {
        let unit = unit_for(input, file).map(|unit| unit.root.as_str());
        let explicit = explicit_paths
            .get(app)
            .into_iter()
            .flatten()
            .copied()
            .filter(|path| within(file, path))
            .max_by_key(|path| path.len());
        if explicit.is_some_and(|path| unit.is_none_or(|root| path.len() >= root.len())) {
            return explicit;
        }
        unit.filter(|root| !executable_roots.contains(root))
    };
    let libraries = library_membership(index, &files, &active, &library_path, &explicit_paths);
    let owners = |id: &str| {
        index
            .get(id)
            .filter(|s| diagram_symbol(s, index))
            .into_iter()
            .flat_map(|s| &s.origins)
            .flat_map(|site| {
                let file = site.file.as_str();
                let local = files
                    .get(file)
                    .into_iter()
                    .flatten()
                    .filter(|app| active.contains(*app))
                    .copied();
                let library = libraries
                    .get(file)
                    .into_iter()
                    .flat_map(|owners| owners.keys().copied());
                local.chain(library)
            })
            .collect::<BTreeSet<_>>()
    };

    let mut links = Vec::new();
    for call in &index.knowledge.http_calls {
        if index.is_test(&call.owner) {
            continue;
        }
        let apps = owners(&call.owner);
        if apps.is_empty() {
            continue;
        }
        let routes =
            nodex_core::knowledge::global::matching_http_routes(call, &index.knowledge.routes);
        if !routes.is_empty() {
            if let [route] = routes.as_slice() {
                let targets = owners(&route.handler);
                if targets.len() == 1 {
                    for app in apps {
                        links.push(Link {
                            from: app.into(),
                            to: (*targets.first().unwrap()).into(),
                            from_symbol: Some(call.owner.clone()),
                            to_symbol: Some(route.handler.clone()),
                            kind: "HTTP",
                            label: format!("{} {}", call.method, route.path),
                            confidence: Confidence::Inferred,
                            provenance: Provenance::Inferred,
                            evidence: ArchitectureEvidence::at(
                                format!(
                                    "{} {} matches {} {} ({})",
                                    call.method,
                                    call.url,
                                    route.method,
                                    route.path,
                                    route.framework
                                ),
                                Some(&call.span),
                                Some(&call.owner),
                            ),
                        });
                    }
                    continue;
                }
            }
            push_evidence(
                &mut result.pending,
                ArchitectureEvidence::at(
                    format!(
                        "{} {} needs a confirmed route and application",
                        call.method, call.url
                    ),
                    Some(&call.span),
                    Some(&call.owner),
                ),
            );
            continue;
        }
        let Some((protocol, host)) = external_host(&call.url) else {
            push_evidence(
                &mut result.pending,
                ArchitectureEvidence::at(
                    "HTTP destination needs confirmation",
                    Some(&call.span),
                    Some(&call.owner),
                ),
            );
            continue;
        };
        let id = format!("host:{host}");
        elements.entry(id.clone()).or_insert_with(|| {
            let mut node = ArchitectureNode::new(
                id.clone(),
                host,
                ArchitectureKind::External,
                Provenance::Inferred,
            );
            node.technology = Some(protocol.into());
            node.evidence.push(ArchitectureEvidence::at(
                "Host literal used by an HTTP call",
                Some(&call.span),
                Some(&call.owner),
            ));
            node
        });
        for app in apps {
            links.push(Link {
                from: app.into(),
                to: id.clone(),
                from_symbol: Some(call.owner.clone()),
                to_symbol: None,
                kind: "HTTP",
                label: format!("{} {protocol}", call.method),
                confidence: Confidence::Inferred,
                provenance: Provenance::Inferred,
                evidence: ArchitectureEvidence::at(
                    "Calls the identified host",
                    Some(&call.span),
                    Some(&call.owner),
                ),
            });
        }
    }
    for symbol in &index.knowledge.symbols {
        if symbol.test || !symbol.capabilities.iter().any(|c| c == "LOCAL_STORAGE") {
            continue;
        }
        for app in owners(&symbol.symbol) {
            let id = format!("browser-store:{app}");
            let evidence = ArchitectureEvidence::at(
                "Uses browser local storage",
                index.get(&symbol.symbol).and_then(|s| s.origins.first()),
                Some(&symbol.symbol),
            );
            let node = elements.entry(id.clone()).or_insert_with(|| {
                let mut node = ArchitectureNode::new(
                    id.clone(),
                    format!("{} local storage", elements_name(&result.applications, app)),
                    ArchitectureKind::Store,
                    Provenance::Detected,
                );
                node.technology = Some("Web Storage".into());
                node
            });
            push_evidence(&mut node.evidence, evidence.clone());
            links.push(Link {
                from: app.into(),
                to: id,
                from_symbol: Some(symbol.symbol.clone()),
                to_symbol: None,
                kind: "Storage",
                label: "Uses local storage".into(),
                confidence: Confidence::Exact,
                provenance: Provenance::Detected,
                evidence,
            });
        }
    }
    add_database_links(
        index,
        config,
        &aliases,
        &owners,
        &mut elements,
        &mut links,
        &mut result.pending,
    );
    result.stores = elements
        .values()
        .filter(|n| n.kind == ArchitectureKind::Store)
        .cloned()
        .collect();
    let mut store_aliases = BTreeMap::new();
    for store in &result.stores {
        for (alias, app) in &aliases {
            if store.id.strip_prefix("browser-store:") == Some(app) {
                store_aliases.insert(format!("browser-store:{alias}"), store.id.as_str());
            }
            if let Some(connection) = store
                .id
                .strip_prefix(&format!("database:{}:{app}:", app.len()))
            {
                store_aliases.insert(
                    format!("database:{}:{alias}:{connection}", alias.len()),
                    store.id.as_str(),
                );
            }
        }
    }
    result.aliases = aliases
        .iter()
        .chain(&store_aliases)
        .map(|(from, to)| (from.clone(), (*to).into()))
        .collect();
    let excluded_stores: BTreeSet<_> = config
        .excluded_stores
        .iter()
        .map(|id| store_aliases.get(id).copied().unwrap_or(id))
        .collect();
    elements.retain(|id, node| {
        node.kind != ArchitectureKind::Store || !excluded_stores.contains(id.as_str())
    });
    links.retain(|link| elements.contains_key(&link.from) && elements.contains_key(&link.to));
    for relation in &config.relations {
        let from = result.aliases.get(&relation.from).unwrap_or(&relation.from);
        let to = result.aliases.get(&relation.to).unwrap_or(&relation.to);
        if elements.contains_key(from) && elements.contains_key(to) {
            links.push(Link {
                from: from.clone(),
                to: to.clone(),
                from_symbol: None,
                to_symbol: None,
                kind: "Declared",
                label: relation.label.clone(),
                confidence: Confidence::Exact,
                provenance: Provenance::Declared,
                evidence: ArchitectureEvidence::at(
                    "Relation declared in local architecture settings",
                    None,
                    None,
                ),
            });
        } else {
            push_evidence(
                &mut result.pending,
                ArchitectureEvidence::at(
                    format!(
                        "A declared relation has an unavailable endpoint: {}",
                        relation.label
                    ),
                    None,
                    None,
                ),
            );
        }
    }
    let mut relations = Relations::default();
    match request.view {
        ArchitectureView::Context => {
            result.nodes.push(system);
            result.nodes.extend(
                elements
                    .values()
                    .filter(|n| {
                        matches!(n.kind, ArchitectureKind::Actor | ArchitectureKind::External)
                    })
                    .cloned(),
            );
            let project_endpoint = |id: &String| match elements[id].kind {
                ArchitectureKind::Application | ArchitectureKind::Store => SYSTEM,
                _ => elements[id].id.as_str(),
            };
            for link in &links {
                add_link(
                    &mut relations,
                    link,
                    project_endpoint(&link.from),
                    project_endpoint(&link.to),
                );
            }
        }
        ArchitectureView::Containers => {
            result.nodes.push(system);
            let internal = |n: &ArchitectureNode| {
                matches!(
                    n.kind,
                    ArchitectureKind::Application | ArchitectureKind::Store
                )
            };
            result.nodes.extend(
                elements
                    .values()
                    .filter(|n| internal(n))
                    .cloned()
                    .map(|mut n| {
                        n.parent = Some(SYSTEM.into());
                        n
                    }),
            );
            result.nodes.extend(
                elements
                    .values()
                    .filter(|n| n.kind != ArchitectureKind::System && !internal(n))
                    .cloned(),
            );
            for link in &links {
                add_link(&mut relations, link, &link.from, &link.to);
            }
            if active.is_empty() {
                result.warnings.push(
                    "Declare an application or open a project with an identified entry point."
                        .into(),
                );
            }
        }
        ArchitectureView::Components => {
            let focus = request
                .container
                .as_deref()
                .map(|id| aliases.get(id).copied().unwrap_or(id))
                .filter(|id| active.contains(id))
                .ok_or_else(|| UmlError::NotFound("application for the component view".into()))?;
            let application = &elements[focus];
            result.title = format!("C4 · {}", application.name);
            result.nodes.push(application.clone());

            let root = apps.iter().find(|app| app.node.id == focus).unwrap().root();
            let mut components = BTreeMap::new();
            let mut symbols = BTreeMap::new();
            for symbol in index
                .graph
                .symbols
                .iter()
                .filter(|s| diagram_symbol(s, index))
            {
                for site in &symbol.origins {
                    let file = site.file.as_str();
                    let path = if files.get(file).is_some_and(|owners| owners.contains(focus)) {
                        module_path(file, root)
                    } else if let Some(path) =
                        libraries.get(file).and_then(|owners| owners.get(focus))
                    {
                        (*path).to_owned()
                    } else {
                        continue;
                    };
                    let (node, component_files) =
                        components.entry(path.clone()).or_insert_with(|| {
                            let mut node = ArchitectureNode::new(
                                format!("component:{focus}:{path}"),
                                leaf(&path).into(),
                                ArchitectureKind::Component,
                                Provenance::Inferred,
                            );
                            node.parent = Some(focus.into());
                            node.path = Some(path);
                            (node, BTreeSet::new())
                        });
                    component_files.insert(file);
                    if symbol.kind.is_type() || symbol.has_flow {
                        push_evidence(
                            &mut node.evidence,
                            ArchitectureEvidence::at(&symbol.name, Some(site), Some(&symbol.id)),
                        );
                    }
                    symbols
                        .entry(symbol.id.as_str())
                        .or_insert_with(|| node.id.clone());
                }
            }
            result
                .nodes
                .extend(components.into_values().map(|(mut node, files)| {
                    let paths: Vec<String> = files.into_iter().map(str::to_owned).collect();
                    node.code = Some(if let [path] = paths.as_slice() {
                        UmlScope::File { path: path.clone() }
                    } else {
                        UmlScope::Files { paths }
                    });
                    node
                }));
            let component = |id: &str| symbols.get(id).map(String::as_str);
            for edge in index.relations().filter(|e| static_relation(e.kind)) {
                if let (Some(a), Some(b)) = (component(&edge.from), component(&edge.to)) {
                    let confidence = edge.confidence.unwrap_or(Confidence::Exact);
                    let kind = format!("{:?}", edge.kind);
                    relations.add(
                        a,
                        b,
                        &kind,
                        &kind,
                        confidence,
                        from_confidence(confidence),
                        edge_evidence(index, edge),
                    );
                }
            }
            let mut neighbors = BTreeSet::new();
            for link in links.iter().filter(|l| l.from == focus || l.to == focus) {
                let a = if link.from == focus {
                    link.from_symbol
                        .as_deref()
                        .and_then(component)
                        .unwrap_or(focus)
                } else {
                    &link.from
                };
                let b = if link.to == focus {
                    link.to_symbol
                        .as_deref()
                        .and_then(component)
                        .unwrap_or(focus)
                } else {
                    &link.to
                };
                if link.from != focus {
                    neighbors.insert(&link.from);
                }
                if link.to != focus {
                    neighbors.insert(&link.to);
                }
                add_link(&mut relations, link, a, b);
            }
            result
                .nodes
                .extend(neighbors.into_iter().map(|id| elements[id].clone()));
        }
        ArchitectureView::Layers => unreachable!(),
    }
    for route in &index.knowledge.routes {
        if !owners(&route.handler).is_empty() {
            continue;
        }
        push_evidence(
            &mut result.pending,
            ArchitectureEvidence::at(
                format!(
                    "{} {} has no associated application",
                    route.method, route.path
                ),
                Some(&route.span),
                Some(&route.handler),
            ),
        );
    }
    result.finish(relations);
    Ok(result)
}

fn library_membership<'a>(
    index: &Index<'a>,
    files: &BTreeMap<&'a str, BTreeSet<&'a str>>,
    active: &BTreeSet<&'a str>,
    library_path: &impl Fn(&str, &str) -> Option<&'a str>,
    explicit_paths: &BTreeMap<&'a str, BTreeSet<&'a str>>,
) -> BTreeMap<&'a str, BTreeMap<&'a str, &'a str>> {
    let mut dependencies = BTreeMap::<&str, BTreeSet<&str>>::new();
    for (from, to) in exact_file_dependencies(index) {
        if files.get(to).is_some_and(BTreeSet::is_empty) {
            dependencies.entry(from).or_default().insert(to);
        }
    }
    let mut app_files = BTreeMap::<&str, Vec<&str>>::new();
    for (&file, owners) in files {
        for app in owners.intersection(active) {
            app_files.entry(app).or_default().push(file);
        }
    }
    let mut libraries = BTreeMap::<&str, BTreeMap<&str, &str>>::new();
    for &app in active {
        let outgoing = |file| {
            dependencies
                .get(file)
                .into_iter()
                .flatten()
                .filter_map(|to| library_path(to, app))
        };
        let mut visit: Vec<_> = explicit_paths
            .get(app)
            .into_iter()
            .flatten()
            .copied()
            .collect();
        visit.extend(
            app_files
                .get(app)
                .into_iter()
                .flatten()
                .flat_map(|file| outgoing(*file)),
        );
        let mut seen = BTreeSet::new();
        while let Some(path) = visit.pop() {
            if !seen.insert(path) {
                continue;
            }
            for (&file, owners) in files
                .range(path..)
                .take_while(|(file, _)| file.starts_with(path))
            {
                if owners.is_empty() && library_path(file, app) == Some(path) {
                    libraries.entry(file).or_default().insert(app, path);
                    visit.extend(outgoing(file));
                }
            }
        }
    }
    libraries
}

fn add_database_links<'a>(
    index: &Index<'_>,
    config: &ArchitectureConfig,
    aliases: &BTreeMap<String, &'a str>,
    owners: &impl Fn(&str) -> BTreeSet<&'a str>,
    elements: &mut BTreeMap<String, ArchitectureNode>,
    links: &mut Vec<Link>,
    pending: &mut Vec<ArchitectureEvidence>,
) {
    let declared: BTreeSet<_> = config
        .relations
        .iter()
        .filter(|relation| {
            relation.to.starts_with("store:")
                && elements
                    .get(&relation.to)
                    .is_some_and(|n| n.kind == ArchitectureKind::Store)
                && !config.excluded_stores.contains(&relation.to)
        })
        .map(|relation| {
            aliases
                .get(&relation.from)
                .copied()
                .unwrap_or(&relation.from)
        })
        .collect();
    for fact in index
        .knowledge
        .config
        .iter()
        .filter(|fact| fact.source == "database-connection")
    {
        for app in owners(&fact.owner).difference(&declared).copied() {
            let id = format!("database:{}:{app}:{}", app.len(), fact.key);
            let connection = fact
                .key
                .strip_prefix("ConnectionStrings:")
                .unwrap_or(&fact.key);
            let name = format!("{} · {connection}", elements[app].name);
            let node = elements.entry(id.clone()).or_insert_with(|| {
                ArchitectureNode::new(
                    id.clone(),
                    name,
                    ArchitectureKind::Store,
                    Provenance::Inferred,
                )
            });
            let evidence = ArchitectureEvidence::at(
                "Named connection passed to the database provider",
                Some(&fact.span),
                Some(&fact.owner),
            );
            push_evidence(&mut node.evidence, evidence.clone());
            links.push(Link {
                from: app.into(),
                to: id,
                from_symbol: Some(fact.owner.clone()),
                to_symbol: None,
                kind: "Storage",
                label: "Configures database access".into(),
                confidence: Confidence::Inferred,
                provenance: Provenance::Inferred,
                evidence,
            });
        }
    }
    // A configured provider establishes its own connection. An operation still
    // needs evidence tying it to a particular context before it can use that link.
    let mut reported = BTreeSet::new();
    for symbol in index.knowledge.symbols.iter().filter(|s| {
        !s.test
            && s.capabilities
                .iter()
                .any(|c| matches!(c.as_str(), "DB_READ" | "DB_WRITE"))
    }) {
        for app in owners(&symbol.symbol).difference(&declared).copied() {
            if reported.insert(app) {
                push_evidence(
                    pending,
                    ArchitectureEvidence::at(
                        format!(
                            "Database access in {} needs an associated data store",
                            elements[app].name
                        ),
                        index.get(&symbol.symbol).and_then(|s| s.origins.first()),
                        Some(&symbol.symbol),
                    ),
                );
            }
        }
    }
}

fn elements_name<'a>(nodes: &'a [ArchitectureNode], id: &str) -> &'a str {
    nodes
        .iter()
        .find(|n| n.id == id)
        .map_or("Application", |n| &n.name)
}

fn add_link(relations: &mut Relations, link: &Link, a: &str, b: &str) {
    relations.add(
        a,
        b,
        link.kind,
        &link.label,
        link.confidence,
        link.provenance,
        link.evidence.clone(),
    );
}

fn external_host(url: &str) -> Option<(&'static str, String)> {
    let (protocol, rest) = if let Some(rest) = url.strip_prefix("https://") {
        ("HTTPS", rest)
    } else {
        ("HTTP", url.strip_prefix("http://")?)
    };
    let authority = rest.split(['/', '?', '#']).next()?;
    let domain = authority.split(':').next()?.to_ascii_lowercase();
    if domain == "localhost"
        || domain.ends_with(".localhost")
        || !domain.contains('.')
        || domain.parse::<std::net::IpAddr>().is_ok()
        || !authority
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b':'))
    {
        return None;
    }
    Some((protocol, domain))
}
