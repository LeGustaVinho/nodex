//! Unity-specific knowledge: engine base classes, lifecycle messages, coroutine waits.

/// Returns the Unity base class name if `base` is one of the engine's scriptable bases.
pub fn behaviour_base(base: &str) -> Option<&'static str> {
    let short = base.rsplit('.').next().unwrap_or(base);
    [
        "MonoBehaviour",
        "ScriptableObject",
        "NetworkBehaviour",
        "StateMachineBehaviour",
        "Editor",
        "EditorWindow",
    ]
    .into_iter()
    .find(|b| *b == short)
}

/// Lifecycle messages the engine calls, with a plain-language description of when.
const MESSAGES: &[(&str, &str)] = &[
    ("Awake", "when the object is created"),
    ("OnEnable", "when the object becomes enabled"),
    ("Start", "before the first frame"),
    ("Update", "every frame"),
    ("FixedUpdate", "every physics step"),
    ("LateUpdate", "every frame, after all Update calls"),
    ("OnDisable", "when the object becomes disabled"),
    ("OnDestroy", "when the object is destroyed"),
    ("OnApplicationQuit", "when the application quits"),
    ("OnApplicationPause", "when the application pauses or resumes"),
    ("OnValidate", "when a value changes in the Inspector"),
    ("Reset", "when the component is reset in the Editor"),
    ("OnGUI", "for every immediate-mode GUI event"),
    ("OnDrawGizmos", "when the Scene view draws gizmos"),
    ("OnDrawGizmosSelected", "when the Scene view draws gizmos for the selection"),
    ("OnCollisionEnter", "when a collision starts"),
    ("OnCollisionStay", "every physics step while colliding"),
    ("OnCollisionExit", "when a collision ends"),
    ("OnCollisionEnter2D", "when a 2D collision starts"),
    ("OnCollisionStay2D", "every physics step while colliding (2D)"),
    ("OnCollisionExit2D", "when a 2D collision ends"),
    ("OnTriggerEnter", "when something enters the trigger"),
    ("OnTriggerStay", "every physics step while something is inside the trigger"),
    ("OnTriggerExit", "when something leaves the trigger"),
    ("OnTriggerEnter2D", "when something enters the 2D trigger"),
    ("OnTriggerStay2D", "while something is inside the 2D trigger"),
    ("OnTriggerExit2D", "when something leaves the 2D trigger"),
    ("OnMouseDown", "when the mouse is pressed on the object"),
    ("OnMouseUp", "when the mouse is released"),
    ("OnMouseEnter", "when the mouse enters the object"),
    ("OnMouseExit", "when the mouse leaves the object"),
    ("OnBecameVisible", "when any camera starts seeing the object"),
    ("OnBecameInvisible", "when no camera sees the object anymore"),
    ("OnAnimatorMove", "when the Animator moves the object"),
    ("OnRenderObject", "after the camera renders the scene"),
];

/// If `name` is a Unity lifecycle message, returns its canonical name.
pub fn lifecycle_message(name: &str) -> Option<&'static str> {
    MESSAGES.iter().find(|(n, _)| *n == name).map(|(n, _)| *n)
}

/// Plain-language description of when the engine calls `message`.
pub fn lifecycle_description(message: &str) -> &'static str {
    MESSAGES
        .iter()
        .find(|(n, _)| *n == message)
        .map(|(_, d)| *d)
        .unwrap_or("when the engine sends this message")
}

/// Fields shown in the Inspector: `[SerializeField]` or public non-static fields.
pub fn is_inspector_field(flavors: &[String]) -> bool {
    let has = |f: &str| flavors.iter().any(|x| x == f);
    has("attr:SerializeField")
        || has("attr:SerializeReference")
        || (has("public") && !has("static") && !has("const") && !has("readonly") && !has("attr:NonSerialized") && !has("attr:HideInInspector"))
}

/// Describes a coroutine wait (`yield return new WaitForSeconds(0.1f)`), if the yielded
/// expression is one. `type_name` is the constructed type; `args` is the argument text.
pub fn describe_wait(type_name: Option<&str>, args: &str, is_null: bool) -> Option<String> {
    if is_null {
        return Some("wait one frame".into());
    }
    let ty = type_name?;
    let args = args.trim();
    Some(match ty {
        "WaitForSeconds" => format!("wait {} seconds", args.trim_end_matches(['f', 'F'])),
        "WaitForSecondsRealtime" => format!("wait {} real-time seconds", args.trim_end_matches(['f', 'F'])),
        "WaitForEndOfFrame" => "wait until the frame finishes rendering".into(),
        "WaitForFixedUpdate" => "wait for the next physics step".into(),
        "WaitUntil" => format!("wait until {args}"),
        "WaitWhile" => format!("wait while {args}"),
        _ => return None,
    })
}
