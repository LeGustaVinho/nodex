use super::*;

pub(super) fn build(
    index: &Index<'_>,
    input: &UmlInput<'_>,
    request: &ArchitectureRequest,
    config: &ArchitectureConfig,
    apps: &[applications::Application],
) -> Result<ArchitectureDiagram, UmlError> {
    let scope = Scoped::resolve(index, &request.scope)?;
    let roots: BTreeSet<&str> = input
        .units
        .iter()
        .map(|u| u.root.as_str())
        .chain(apps.iter().map(|app| app.root()))
        .collect();
    let group_for_file = |file: &str| {
        let root = roots
            .iter()
            .copied()
            .filter(|root| within(file, root))
            .max_by_key(|r| r.len())
            .unwrap_or("");
        module_path(file, root)
    };
    let site_in_scope = |site: &Span| {
        (scope.files.is_empty() || scope.files.contains(&site.file))
            && scope.folder.as_ref().is_none_or(|f| within(&site.file, f))
    };
    let mut groups: BTreeMap<String, Vec<(&Symbol, &Span)>> = BTreeMap::new();
    let mut visible = BTreeSet::new();
    let mut group_by_symbol = BTreeMap::new();
    for symbol in index
        .graph
        .symbols
        .iter()
        .filter(|s| diagram_symbol(s, index))
    {
        for site in &symbol.origins {
            let group = group_for_file(&site.file);
            groups
                .entry(group.clone())
                .or_default()
                .push((symbol, site));
            if scope.symbols.contains(&symbol.id) && site_in_scope(site) {
                visible.insert(group.clone());
                group_by_symbol.entry(symbol.id.as_str()).or_insert(group);
            }
        }
    }

    let mut result = ArchitectureDiagram::new(format!("Layers · {}", scope.title));
    let mut endpoints = BTreeMap::new();
    let mut bands: BTreeMap<Layer, Vec<ArchitectureNode>> = BTreeMap::new();
    for group in visible {
        let members = &groups[&group];
        let (layer, why) = classify(index, &group, members, config);
        let provenance = if config.layers.contains_key(&group) {
            Provenance::Declared
        } else {
            Provenance::Inferred
        };
        let band = format!("layer:{layer:?}");
        let mut node = ArchitectureNode::new(
            format!("module:{group}"),
            leaf(&group).into(),
            ArchitectureKind::Module,
            provenance,
        );
        node.layer = Some(layer);
        node.parent = Some(band.clone());
        node.path = Some(group.clone());
        node.evidence
            .push(ArchitectureEvidence::at(why, None, None));
        for (s, span) in members
            .iter()
            .filter(|(s, _)| s.has_flow || s.kind.is_type())
        {
            push_evidence(
                &mut node.evidence,
                ArchitectureEvidence::at(&s.name, Some(span), Some(&s.id)),
            );
        }
        let expanded = request.expanded.contains(&layer);
        endpoints.insert(group, if expanded { node.id.clone() } else { band });
        bands.entry(layer).or_default().push(node);
    }
    let mut diagram_relations = Relations::default();
    let mut modules = Vec::new();
    for layer in Layer::ALL {
        let entries = bands.remove(&layer).unwrap_or_default();
        if layer == Layer::Unclassified && entries.is_empty() {
            continue;
        }
        let mut band = ArchitectureNode::new(
            format!("layer:{layer:?}"),
            format!("{layer:?}"),
            ArchitectureKind::Layer,
            Provenance::Detected,
        );
        band.layer = Some(layer);
        band.evidence.push(ArchitectureEvidence::at(
            format!("{} module(s)", entries.len()),
            None,
            None,
        ));
        result.nodes.push(band);
        if request.expanded.contains(&layer) {
            modules.extend(entries.iter().cloned());
        }
        result.modules.extend(entries);
    }
    result.nodes.extend(modules);

    let target = |id: &str| {
        if let Some(endpoint) = group_by_symbol.get(id).and_then(|g| endpoints.get(g)) {
            return Some(endpoint.as_str());
        }
        let symbol = index.get(id)?;
        if !matches!(symbol.kind, SymbolKind::Namespace | SymbolKind::Package)
            || !scope.symbols.contains(id)
            || index.is_test(id)
        {
            return None;
        }
        // Namespaces can span modules. Lift them only when their origins agree
        // on a visible endpoint at the current expansion level.
        let mut candidates = symbol
            .origins
            .iter()
            .filter(|s| site_in_scope(s))
            .map(|s| endpoints.get(&group_for_file(&s.file)).map(String::as_str));
        let one = candidates.next()??;
        candidates.all(|other| other == Some(one)).then_some(one)
    };
    for edge in index.relations().filter(|e| static_relation(e.kind)) {
        let source = match (&edge.kind, &edge.site) {
            (StructEdgeKind::Imports, Some(site))
                if scope.symbols.contains(&edge.from) && site_in_scope(site) =>
            {
                endpoints
                    .get(&group_for_file(&site.file))
                    .map(String::as_str)
            }
            (StructEdgeKind::Imports, Some(_)) => None,
            _ => target(&edge.from),
        };
        if let (Some(source), Some(dest)) = (source, target(&edge.to)) {
            let confidence = edge.confidence.unwrap_or(Confidence::Exact);
            let kind = format!("{:?}", edge.kind);
            diagram_relations.add(
                source,
                dest,
                &kind,
                &kind,
                confidence,
                from_confidence(confidence),
                edge_evidence(index, edge),
            );
        }
    }
    for unit in input.units {
        for reference in &unit.refs {
            // A manifest reference identifies units. Only lift it when each unit has one visible endpoint.
            let endpoint = |root: &str| {
                let mut matching = endpoints
                    .iter()
                    .filter(|(path, _)| {
                        input
                            .units
                            .iter()
                            .filter(|u| within(path, &u.root))
                            .max_by_key(|u| u.root.len())
                            .is_some_and(|u| u.root == root)
                    })
                    .map(|(_, endpoint)| endpoint.as_str());
                let one = matching.next()?;
                matching.all(|other| other == one).then_some(one)
            };
            if let (Some(a), Some(b)) = (endpoint(&unit.root), endpoint(reference)) {
                diagram_relations.add(
                    a,
                    b,
                    "Manifest",
                    "References",
                    Confidence::Exact,
                    Provenance::Detected,
                    ArchitectureEvidence::file(
                        format!("{} references {reference}", unit.name),
                        &unit.manifest,
                    ),
                );
            }
        }
    }
    result.finish(diagram_relations);
    Ok(result)
}

fn classify(
    index: &Index<'_>,
    path: &str,
    members: &[(&Symbol, &Span)],
    config: &ArchitectureConfig,
) -> (Layer, String) {
    if let Some(layer) = config.layers.get(path) {
        return (*layer, "Layer selected for this module".into());
    }

    let mut signals = BTreeSet::new();
    for (s, _) in members {
        if index.routed.contains_key(&s.id)
            || matches!(
                entry(index, &s.id),
                Some(EntryKind::Route | EntryKind::Ui | EntryKind::Command)
            )
            || s.flavors.iter().any(|f| {
                matches!(
                    f.as_str(),
                    "react:component" | "vue:component" | "angular:component"
                )
            })
        {
            signals.insert(Layer::Interface);
        }
        if s.kind.is_type()
            && index.children.get(s.id.as_str()).is_some_and(|kids| {
                kids.iter().any(|child| {
                    child
                        .type_text
                        .as_ref()
                        .is_some_and(|t| t.starts_with("DbSet<"))
                })
            })
        {
            signals.insert(Layer::Infrastructure);
        }
    }
    match signals.len() {
        1 => {
            return (
                *signals.first().unwrap(),
                "Responsibility detected in the module's declarations".into(),
            );
        }
        2.. => {
            return (
                Layer::Unclassified,
                "The module contains conflicting responsibility signals".into(),
            );
        }
        _ => {}
    }

    let convention = |segment: &str| {
        let stem = segment
            .rsplit_once('.')
            .map_or(segment, |(stem, extension)| {
                if matches!(extension, "cs" | "rs" | "ts" | "tsx" | "js" | "jsx") {
                    stem
                } else {
                    segment
                }
            });
        match stem.to_ascii_lowercase().as_str() {
            "interface" | "controllers" | "routes" | "views" | "ui" | "cli" => {
                Some(Layer::Interface)
            }
            "application" | "services" | "usecases" | "use-cases" => Some(Layer::Application),
            "domain" | "entities" | "valueobjects" | "value-objects" => Some(Layer::Domain),
            "infra" | "infrastructure" | "data" | "db" | "persistence" | "repositories"
            | "adapters" => Some(Layer::Infrastructure),
            _ => None,
        }
    };
    if let Some(layer) = path.rsplit('/').find_map(convention) {
        return (layer, format!("Folder or module naming convention: {path}"));
    }
    for (s, _) in members {
        if let Some(namespace) = index.namespace_of(&s.id) {
            if let Some(layer) = namespace
                .qualified_name
                .split(['.', ':'])
                .filter_map(convention)
                .last()
            {
                signals.insert(layer);
            }
        }
    }
    if signals.len() == 1 {
        return (
            *signals.first().unwrap(),
            "Namespace naming convention".into(),
        );
    }
    (
        Layer::Unclassified,
        "A layer could not be inferred from the declarations or names".into(),
    )
}
