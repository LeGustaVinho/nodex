//! Reads declared types (`List<Order>?`, `Option<Box<Node>>`, `Map<string, User>`) to find what a
//! field refers to and with which multiplicity and ownership.

/// How a field holds what it refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holding {
    /// In a box: owned, on the heap.
    Boxed,
    /// Shared ownership (`Rc`, `Arc`): aggregated.
    Shared,
    /// Written plainly (`Order`): by value in Rust and for value types, a handle to an object in
    /// C# and TypeScript.
    Plain,
    /// Borrowed (`&T`, `&mut T`).
    Borrowed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeShape {
    /// The referred type, without wrappers or generic arguments (`Order`).
    pub target: String,
    pub many: bool,
    pub optional: bool,
    /// Key type of a map (`Dictionary<int, Order>` → `int`).
    pub key: Option<String>,
    pub holding: Holding,
}

impl TypeShape {
    /// UML multiplicity of the target end.
    pub fn multiplicity(&self) -> &'static str {
        match (self.many, self.optional) {
            (true, _) => "0..*",
            (false, true) => "0..1",
            (false, false) => "1",
        }
    }
}

const COLLECTIONS: &[&str] = &[
    "List", "IList", "IEnumerable", "ICollection", "IReadOnlyList", "IReadOnlyCollection", "Collection", "ObservableCollection",
    "HashSet", "ISet", "SortedSet", "Set", "ReadonlySet", "Vec", "VecDeque", "LinkedList", "BTreeSet", "Array", "ReadonlyArray",
    "ImmutableList", "ImmutableArray", "Queue", "Stack", "IAsyncEnumerable", "DbSet", "IQueryable",
];
const MAPS: &[&str] =
    &["Dictionary", "IDictionary", "IReadOnlyDictionary", "ConcurrentDictionary", "SortedDictionary", "HashMap", "BTreeMap", "Map", "ReadonlyMap", "Record", "ImmutableDictionary"];
const WRAPPERS: &[&str] = &[
    "Task", "ValueTask", "Lazy", "Promise", "RefCell", "Cell", "Mutex", "RwLock", "Observable", "Signal", "WritableSignal", "Ref",
    "Readonly", "Partial", "Required", "Nullable", "IOptions", "Func", "Pin", "Cow", "OnceCell", "OnceLock", "BehaviorSubject", "Subject",
];
const OPTIONALS: &[&str] = &["Option", "Optional", "Maybe"];

/// Reads a declared type. `None` for an empty text.
pub fn analyze(text: &str) -> Option<TypeShape> {
    let mut shape = TypeShape { target: String::new(), many: false, optional: false, key: None, holding: Holding::Plain };
    let mut t = text.trim().trim_start_matches("readonly ").trim().to_owned();
    if t.is_empty() {
        return None;
    }
    // TypeScript unions with undefined/null.
    if t.contains('|') {
        let parts: Vec<&str> = top_level_split(&t, '|').into_iter().map(str::trim).filter(|p| !p.is_empty()).collect();
        let kept: Vec<&str> = parts.iter().copied().filter(|p| !matches!(*p, "undefined" | "null" | "void")).collect();
        if kept.len() < parts.len() {
            shape.optional = true;
        }
        t = kept.first().copied().unwrap_or("").to_owned();
    }
    let mut first_level = true;
    for _ in 0..8 {
        t = t.trim().to_owned();
        if let Some(inner) = t.strip_suffix('?') {
            shape.optional = true;
            t = inner.to_owned();
            continue;
        }
        if let Some(inner) = t.strip_suffix("[]") {
            shape.many = true;
            t = inner.to_owned();
            continue;
        }
        if let Some(inner) = t.strip_prefix('&') {
            shape.holding = Holding::Borrowed;
            let inner = inner.trim_start();
            let inner = inner.strip_prefix('\'').map(|r| r.split_once(' ').map(|(_, rest)| rest).unwrap_or(r)).unwrap_or(inner);
            t = inner.trim_start_matches("mut ").to_owned();
            first_level = false;
            continue;
        }
        if let Some(inner) = t.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            // Rust slices and arrays: `[T]`, `[T; N]`.
            shape.many = true;
            t = inner.split(';').next().unwrap_or(inner).to_owned();
            continue;
        }
        t = t.trim_start_matches("dyn ").trim_start_matches("impl ").to_owned();
        let (name, args) = split_generic(&t);
        let simple = name.rsplit(['.', ':']).next().unwrap_or(&name).to_owned();
        if args.is_empty() {
            shape.target = simple;
            break;
        }
        if MAPS.contains(&simple.as_str()) && args.len() >= 2 {
            shape.many = true;
            shape.key = Some(args[0].clone());
            t = args[args.len() - 1].clone();
        } else if COLLECTIONS.contains(&simple.as_str()) {
            shape.many = true;
            t = args[0].clone();
        } else if OPTIONALS.contains(&simple.as_str()) {
            shape.optional = true;
            t = args[0].clone();
        } else if simple == "Box" {
            if first_level {
                shape.holding = Holding::Boxed;
            }
            t = args[0].clone();
        } else if matches!(simple.as_str(), "Rc" | "Arc" | "Weak") {
            shape.holding = Holding::Shared;
            t = args[0].clone();
        } else if WRAPPERS.contains(&simple.as_str()) {
            t = args[0].clone();
        } else {
            // A generic type of the project (`Repository<Order>`): the type itself.
            shape.target = simple;
            break;
        }
        first_level = false;
    }
    if shape.target.is_empty() {
        return None;
    }
    Some(shape)
}

/// `Dictionary<int, List<Order>>` → (`Dictionary`, [`int`, `List<Order>`]).
fn split_generic(t: &str) -> (String, Vec<String>) {
    let Some(open) = t.find('<') else { return (t.trim().to_owned(), Vec::new()) };
    let close = t.rfind('>').unwrap_or(t.len());
    if close <= open {
        return (t.trim().to_owned(), Vec::new());
    }
    let inner = &t[open + 1..close];
    let args = top_level_split(inner, ',').into_iter().map(|a| a.trim().to_owned()).filter(|a| !a.is_empty() && !a.starts_with('\'')).collect();
    (t[..open].trim().to_owned(), args)
}

pub fn top_level_split(text: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, ch) in text.char_indices() {
        match ch {
            '<' | '(' | '[' | '{' => depth += 1,
            '>' | ')' | ']' | '}' => depth -= 1,
            c if c == sep && depth == 0 => {
                out.push(&text[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&text[start..]);
    out
}

/// Parameters of a signature in UML form (`id: int, name: string`): C# `int id` is turned
/// around; TypeScript and Rust already read `name: Type`. `self` receivers are dropped.
pub fn uml_parameters(signature: &str, name: &str, csharp: bool) -> String {
    // The parameter list follows the name (attributes before it have parentheses too).
    let open = signature.find(&format!("{name}(")).map(|i| i + name.len()).or_else(|| signature.find(&format!("{name}<")).and_then(|i| signature[i..].find('(').map(|j| i + j)));
    let Some(open) = open.or_else(|| signature.find('(')) else { return String::new() };
    let mut depth = 0;
    let mut close = signature.len();
    for (i, ch) in signature[open..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    close = open + i;
                    break;
                }
            }
            _ => {}
        }
    }
    let inner = &signature[open + 1..close.min(signature.len())];
    let params: Vec<String> = top_level_split(inner, ',')
        .into_iter()
        .map(|p| p.split('=').next().unwrap_or(p).trim().to_owned())
        .filter(|p| !p.is_empty() && !matches!(p.as_str(), "self" | "&self" | "&mut self" | "mut self"))
        .map(|p| {
            if !csharp || p.contains(':') {
                return p.trim_start_matches("public ").trim_start_matches("private ").trim_start_matches("readonly ").to_owned();
            }
            // `this Order order`, `params int[] xs`, `[FromBody] Order order`.
            let p = p.split(']').next_back().unwrap_or(&p).trim();
            match p.rsplit_once(' ') {
                Some((ty, name)) => {
                    let ty = ty.trim_start_matches("this ").trim_start_matches("params ").trim_start_matches("in ");
                    format!("{name}: {ty}")
                }
                None => p.to_owned(),
            }
        })
        .collect();
    params.join(", ")
}

/// Parameters of a signature as (name, type) pairs.
pub fn parameter_types(signature: &str, name: &str, csharp: bool) -> Vec<(String, String)> {
    let params = uml_parameters(signature, name, csharp);
    top_level_split(&params, ',')
        .into_iter()
        .filter_map(|p| {
            let (n, t) = p.split_once(':')?;
            let n = n.trim().trim_start_matches("mut ").trim_end_matches('?').trim();
            Some((n.to_owned(), t.trim().to_owned()))
        })
        .collect()
}

/// The outer name of a type (`DbSet<Order>` → `DbSet`, `System.String` → `String`).
pub fn outer_name(text: &str) -> String {
    let (name, _) = split_generic(text.trim().trim_end_matches('?').trim_end_matches("[]"));
    name.rsplit(['.', ':']).next().unwrap_or(&name).trim_start_matches('&').trim_start_matches("mut ").to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(t: &str) -> (String, bool, bool, Option<String>, Holding) {
        let s = analyze(t).expect("a type");
        (s.target, s.many, s.optional, s.key, s.holding)
    }

    #[test]
    fn reads_collections_maps_optionals_and_ownership() {
        assert_eq!(shape("List<Order>"), ("Order".into(), true, false, None, Holding::Plain));
        assert_eq!(shape("Order?"), ("Order".into(), false, true, None, Holding::Plain));
        assert_eq!(shape("Dictionary<int, Order>"), ("Order".into(), true, false, Some("int".into()), Holding::Plain));
        assert_eq!(shape("Option<Box<Node>>"), ("Node".into(), false, true, None, Holding::Plain));
        assert_eq!(shape("Box<Node>"), ("Node".into(), false, false, None, Holding::Boxed));
        assert_eq!(shape("Arc<Mutex<Cache>>"), ("Cache".into(), false, false, None, Holding::Shared));
        assert!(shape("Vec<Order>").1);
        assert_eq!(shape("&'a Config").0, "Config");
        assert_eq!(shape("Order[] | undefined"), ("Order".into(), true, true, None, Holding::Plain));
        assert_eq!(shape("Task<IEnumerable<Order>>"), ("Order".into(), true, false, None, Holding::Plain));
        assert_eq!(shape("Repository<Order>").0, "Repository");
        assert_eq!(shape("crate::domain::Order").0, "Order");
        assert!(analyze("").is_none());
    }

    #[test]
    fn parameters_read_as_uml() {
        assert_eq!(uml_parameters("public Order Find(int id, string? name = null)", "Find", true), "id: int, name: string?");
        assert_eq!(uml_parameters("withStatus(status: OrderStatus): Order", "withStatus", false), "status: OrderStatus");
        assert_eq!(uml_parameters("pub fn new(&self, id: u32) -> Self", "new", false), "id: u32");
        assert_eq!(uml_parameters("void Add(Dictionary<int, Order> map)", "Add", true), "map: Dictionary<int, Order>");
        assert_eq!(uml_parameters("[HttpPost(\"{id}/cancel\")] public Task<IActionResult> Cancel(Guid id)", "Cancel", true), "id: Guid");
        assert_eq!(parameter_types("void Add(Dictionary<int, Order> map, int n)", "Add", true), vec![("map".into(), "Dictionary<int, Order>".into()), ("n".into(), "int".into())]);
        assert_eq!(outer_name("DbSet<Order>"), "DbSet");
    }
}
