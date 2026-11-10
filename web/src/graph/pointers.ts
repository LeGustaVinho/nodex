/** Rust smart pointers and guards, recognized in pin types: what owning the value through them means. */
const POINTERS: [RegExp, string, string][] = [
  [/^Box</, "Box", "Owned on the heap: one owner, freed when the box is dropped."],
  [/^Rc</, "Rc", "Shared ownership (one thread): the value lives until the last Rc is dropped."],
  [/^Arc</, "Arc", "Shared ownership across threads: the value lives until the last Arc is dropped."],
  [/^Weak</, "Weak", "A non-owning link to an Rc/Arc value: it may already be gone."],
  [/^RefCell</, "RefCell", "Changeable through shared references; borrowing rules are checked while running (a conflict panics)."],
  [/^Cell</, "Cell", "Changeable through shared references by copying values in and out."],
  [/^Mutex</, "Mutex", "One thread at a time: lock() waits for access and returns a guard."],
  [/^RwLock</, "RwLock", "Many readers or one writer at a time."],
  [/^(MutexGuard|RwLock\w*Guard)</, "lock", "Holds the lock: released when this value goes out of scope."],
  [/^RefMut</, "borrow", "A runtime mutable borrow of a RefCell: released when it goes out of scope."],
  [/^&(?:'\w+ )?mut /, "&mut", "This exclusive reference lets you write through it. Reassigning the name that holds the reference is a separate rule."],
];

/** Badge for a Rust smart pointer or guard type, if the pin has one. */
export function pointerBadge(ty: string | null | undefined): { label: string; explain: string } | undefined {
  if (!ty) return undefined;
  const hit = POINTERS.find(([re]) => re.test(ty));
  return hit ? { label: hit[1], explain: hit[2] } : undefined;
}
