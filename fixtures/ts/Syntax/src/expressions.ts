import { EventEmitter } from "node:events";

interface User {
  name: string;
  address?: { city?: string; zip: number };
  tags: string[];
}

export async function expressions(users: User[], user: User | null, emitter: EventEmitter, key: string): Promise<unknown> {
  const t = `hello ${user?.name} and ${users.length + 1}`;
  const city = user?.address?.city ?? "unknown";
  const zip = user!.address!.zip;
  const first = users?.[0]?.tags?.at(0);
  const asserted = user as unknown as Record<string, unknown>;
  const checked = { a: 1 } satisfies Record<string, number>;
  const frozen = ["a", "b"] as const;
  const kind = typeof user === "object" ? user instanceof Object : key in (user ?? {});
  const total = users.filter((u) => u.tags.length > 0).map((u) => u.tags.length).reduce((a, b) => a + b, 0);
  const names = users.map((u) => u.name).join(", ");
  const loose = (key as unknown) == 0 || key != undefined;
  const strict = key === "x" && key !== "y";
  const math = ((1 + 2) * 3) / 4 - (5 % 2) ** 2;
  const bits = (1 << 2) | ((8 >> 1) & ~0) ^ (16 >>> 2);
  const obj = { key, [key]: 1, ...user, method() { return 1; }, get g() { return 2; }, set g(v: number) { void v; } };
  const arr = [1, ...frozen, , 3];
  const fn = function (this: { x: number }) { return this.x; };
  const gen = function* () { yield 1; };
  const asyncArrow = async (n: number) => { await Promise.resolve(n); return n; };
  const instance = new Map<string, number>([["a", 1]]);
  const cls = class Named {};
  const re = /ab+c/gi;
  const big = 10n ** 3n;
  const nothing = void 0;
  const neg = -t.length;
  const not = !user;
  const seq = (users.pop(), users.length);
  const tagged = String.raw`a\nb${t}`;
  const meta = import.meta.url;
  const generic = Array.of<number>;
  const lazy = await import("./modules/index.js");
  delete (obj as Record<string, unknown>)[key];
  emitter.on("change", (value: number) => console.log(value));
  emitter.once("close", () => {});
  emitter.off("change", () => {});
  emitter.emit("change", 1);
  emitter.removeAllListeners("change");
  const promise = fetch(key)
    .then((r) => r.json())
    .then((data) => data.items)
    .catch(() => [])
    .finally(() => console.log("done"));
  user && console.log(user.name);
  user || console.log("no user");
  user ?? console.log("null user");
  return [t, city, zip, first, asserted, checked, kind, total, names, loose, strict, math, bits, arr, fn, gen,
    asyncArrow, instance, cls, re, big, nothing, neg, not, seq, tagged, meta, generic, lazy, promise];
}
