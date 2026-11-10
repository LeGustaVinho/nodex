class Queue<T> {
  count = 0;
}

export function typedCollections(value: string | number) {
  const dictionary = new Map<string, boolean>();
  const nested = new Map<string, Array<Set<number>>>();
  const own = new Queue<string>();
  const { first, second } = { first: true, second: [1, "two"] as const };
  if (typeof value === "string") {
    const narrowed = value;
    console.log(narrowed);
  }
  const converted = [1, 2].map(item => item.toString());
  return [dictionary, nested, own, first, second, converted];
}

enum Phase { Waiting, Ready }
enum Mode { Manual = "manual", Auto = "auto" }

export function enumTypes(phase: Phase, mode: Mode) {
  return [phase, mode];
}

export const typedArrow: (value: number) => number = value => value;
