declare function open(): Disposable;
declare function openAsync(): AsyncDisposable;
declare function done(): void;

export async function statements(xs: number[], o: any, x: number): Promise<number> {
  for (let i = 0, j = 1; i < xs.length; i++, j--) {
    x += i * j;
  }
  for (const n of xs) x += n;
  for await (const [k, v] of Object.entries(o)) {
    console.log(k, v);
  }
  for (const key in o) {
    if (!Object.hasOwn(o, key)) continue;
  }
  for (;;) {
    break;
  }
  outer: while (true) {
    for (const n of xs) {
      if (n > 3) break outer;
      if (n < 0) continue outer;
    }
  }
  do {
    x--;
  } while (x > 0);
  switch (o.kind) {
    case "a":
    case "b":
      x = 1;
      break;
    case "c":
      x = 2;
    case "d": {
      x = 3;
      break;
    }
    default:
      x = 4;
  }
  try {
    throw new Error("boom");
  } catch (e: unknown) {
    if (e instanceof Error) console.log(e.message);
  } finally {
    done();
  }
  try {
    x = JSON.parse("1");
  } catch {
    x = 0;
  }
  {
    using r = open();
    await using q = openAsync();
    void r;
    void q;
  }
  let { a, b: [c, d] = [1, 2], ...rest } = o;
  [a, c] = [c, a];
  ({ a } = o);
  var hoisted = 1;
  x ??= 2;
  x ||= 3;
  x &&= 4;
  x += 1;
  x **= 2;
  o.p.q = 4;
  xs[0] = 5;
  x++;
  --x;
  label: {
    if (x > 100) break label;
    x = 100;
  }
  if (!o) return 0;
  else if (o == null) {
    return 1;
  } else if (typeof o === "string") return 2;
  debugger;
  return a + c + d + hoisted + Object.keys(rest).length;
}

export function guards(value: unknown): value is string {
  return typeof value === "string";
}

export function assertIsDefined<T>(value: T): asserts value is NonNullable<T> {
  if (value === undefined || value === null) {
    throw new Error("not defined");
  }
}

export function timers(): void {
  setTimeout(() => console.log("later"), 100);
  const handle = setInterval(() => console.log("tick"), 1000);
  clearInterval(handle);
  queueMicrotask(() => {});
}

export async function waits(ms: number): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, ms));
  await sleep(ms);
}

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

export function nested(): number {
  function helper(n: number): number {
    return n * 2;
  }
  const square = (n: number): number => {
    return n * n;
  };
  const inline = (n: number) => n + 1;
  class Local {
    value = 1;
  }
  enum Mode {
    On,
    Off,
  }
  return helper(square(inline(new Local().value + Mode.On)));
}
