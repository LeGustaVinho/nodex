use super::applications::{Application, membership, unit_for};
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
    let active: BTreeSet<&str> = apps
        .iter()
        .filter(|a| {
            !config.excluded_apps.contains(&a.node.id)
                && a.legacy_id()
                    .is_none_or(|id| !config.excluded_apps.contains(id))
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

    let membership = membership(index, input, &apps);
    let mut libraries: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for edge in index.relations().filter(|e| {
        static_relation(e.kind) && e.confidence.unwrap_or(Confidence::Exact) == Confidence::Exact
    }) {
        if let (Some(owners), Some(target)) =
            (membership.get(edge.from.as_str()), index.file_of(&edge.to))
            && !membership.contains_key(edge.to.as_str())
            && let Some(unit) = unit_for(input, target)
            && !apps.iter().any(|a| a.root() == unit.root)
        {
            libraries
                .entry(&unit.root)
                .or_default()
                .extend(owners.intersection(&active));
        }
    }
    for item in &config.libraries {
        if active.contains(item.application.as_str()) {
            libraries
                .entry(&item.path)
                .or_default()
                .insert(&item.application);
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
    let owners = |id: &str| {
        if let Some(owners) = membership.get(id) {
            return owners.intersection(&active).copied().collect();
        }
        let Some(file) = index.file_of(id) else {
            return BTreeSet::new();
        };
        libraries
            .iter()
            .filter(|(path, _)| within(file, path))
            .max_by_key(|(path, _)| path.len())
            .map(|(_, owners)| owners.clone())
            .unwrap_or_default()
    };

    let mut links = Vec::new();
    let mut handled_http = BTreeSet::new();
    for edge in index
        .relations()
        .filter(|e| e.kind == StructEdgeKind::HttpCalls)
    {
        if let Some(site) = &edge.site {
            handled_http.insert((site.file.as_str(), site.start_byte));
        }
        let callers = owners(&edge.from);
        if callers.is_empty() {
            continue;
        }
        let confidence = edge.confidence.unwrap_or(Confidence::Exact);
        if confidence == Confidence::Unresolved {
            push_evidence(&mut result.pending, edge_evidence(index, edge));
            continue;
        }
        let targets = owners(&edge.to);
        if targets.len() != 1 {
            push_evidence(&mut result.pending, edge_evidence(index, edge));
            continue;
        }
        let target = *targets.first().unwrap();
        let label = index
            .knowledge
            .routes
            .iter()
            .find(|r| r.handler == edge.to)
            .map(|r| format!("{} {}", r.method, r.path))
            .unwrap_or_else(|| "HTTP".into());
        for app in callers {
            links.push(Link {
                from: app.into(),
                to: target.into(),
                from_symbol: Some(edge.from.clone()),
                to_symbol: Some(edge.to.clone()),
                kind: "HTTP",
                label: label.clone(),
                confidence,
                provenance: from_confidence(confidence),
                evidence: edge_evidence(index, edge),
            });
        }
    }
    for call in &index.knowledge.http_calls {
        if handled_http.contains(&(call.span.file.as_str(), call.span.start_byte))
            || index.is_test(&call.owner)
        {
            continue;
        }
        let apps = owners(&call.owner);
        if apps.is_empty() {
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
    elements.retain(|id, node| {
        node.kind != ArchitectureKind::Store || !config.excluded_stores.contains(id)
    });
    links.retain(|link| elements.contains_key(&link.from) && elements.contains_key(&link.to));
    for relation in &config.relations {
        if elements.contains_key(&relation.from) && elements.contains_key(&relation.to) {
            links.push(Link {
                from: relation.from.clone(),
                to: relation.to.clone(),
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
                .filter(|id| active.contains(id))
                .ok_or_else(|| UmlError::NotFound("application for the component view".into()))?;
            let application = &elements[focus];
            result.title = format!("C4 · {}", application.name);
            result.nodes.push(application.clone());

            let root = application.path.as_deref().unwrap_or("");
            let mut components = BTreeMap::new();
            let mut symbols = BTreeMap::new();
            for symbol in index
                .graph
                .symbols
                .iter()
                .filter(|s| diagram_symbol(s, index) && owners(&s.id).contains(focus))
            {
                let site = &symbol.origins[0];
                let local = membership
                    .get(symbol.id.as_str())
                    .is_some_and(|owners| owners.contains(focus));
                let path = if local {
                    module_path(&site.file, root)
                } else {
                    libraries
                        .keys()
                        .copied()
                        .filter(|p| within(&site.file, p))
                        .max_by_key(|p| p.len())
                        .unwrap_or(&site.file)
                        .to_owned()
                };
                let node = components.entry(path.clone()).or_insert_with(|| {
                    let mut node = ArchitectureNode::new(
                        format!("component:{focus}:{path}"),
                        leaf(&path).into(),
                        ArchitectureKind::Component,
                        Provenance::Inferred,
                    );
                    node.parent = Some(focus.into());
                    node.path = Some(path.clone());
                    node.code = Some(if path == site.file {
                        UmlScope::File { path }
                    } else {
                        UmlScope::Folder { path }
                    });
                    node
                });
                if symbol.kind.is_type() || symbol.has_flow {
                    push_evidence(
                        &mut node.evidence,
                        ArchitectureEvidence::at(&symbol.name, Some(site), Some(&symbol.id)),
                    );
                }
                symbols.insert(symbol.id.as_str(), node.id.clone());
            }
            result.nodes.extend(components.into_values());
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

fn add_database_links<'a>(
    index: &Index<'_>,
    config: &ArchitectureConfig,
    owners: &impl Fn(&str) -> BTreeSet<&'a str>,
    elements: &mut BTreeMap<String, ArchitectureNode>,
    links: &mut Vec<Link>,
    pending: &mut Vec<ArchitectureEvidence>,
) {
    let mut connections = BTreeMap::new();
    for fact in index
        .knowledge
        .config
        .iter()
        .filter(|fact| fact.source == "connection-string")
    {
        for app in owners(&fact.owner) {
            connections
                .entry(app)
                .or_insert_with(BTreeMap::new)
                .insert(fact.key.as_str(), fact);
        }
    }
    let mut accesses = BTreeMap::new();
    for symbol in index.knowledge.symbols.iter().filter(|s| {
        !s.test
            && s.capabilities
                .iter()
                .any(|c| matches!(c.as_str(), "DB_READ" | "DB_WRITE"))
    }) {
        for app in owners(&symbol.symbol) {
            accesses
                .entry(app)
                .or_insert_with(Vec::new)
                .push(&symbol.symbol);
        }
    }
    for (app, symbols) in accesses {
        if config
            .relations
            .iter()
            .any(|r| r.from == app && r.to.starts_with("store:") && elements.contains_key(&r.to))
        {
            continue;
        }
        let Some(fact) = connections
            .get(app)
            .filter(|c| c.len() == 1)
            .and_then(|c| c.values().next())
        else {
            let symbol = symbols[0];
            push_evidence(
                pending,
                ArchitectureEvidence::at(
                    format!(
                        "Database access in {} needs an associated data store",
                        elements[app].name
                    ),
                    index.get(symbol).and_then(|s| s.origins.first()),
                    Some(symbol),
                ),
            );
            continue;
        };
        let id = format!("database:{}:{app}:{}", app.len(), fact.key);
        let connection = fact
            .key
            .strip_prefix("ConnectionStrings:")
            .unwrap_or(&fact.key);
        let name = format!("{} · {connection}", elements[app].name);
        let mut node = ArchitectureNode::new(
            id.clone(),
            name,
            ArchitectureKind::Store,
            Provenance::Inferred,
        );
        node.evidence.push(ArchitectureEvidence::at(
            "Named connection string read by the application",
            Some(&fact.span),
            Some(&fact.owner),
        ));
        elements.insert(id.clone(), node);
        for symbol in symbols {
            links.push(Link {
                from: app.into(),
                to: id.clone(),
                from_symbol: Some(symbol.clone()),
                to_symbol: None,
                kind: "Storage",
                label: "Uses database".into(),
                confidence: Confidence::Inferred,
                provenance: Provenance::Inferred,
                evidence: ArchitectureEvidence::at(
                    "Database access in an application with one named connection",
                    index.get(symbol).and_then(|s| s.origins.first()),
                    Some(symbol),
                ),
            });
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
