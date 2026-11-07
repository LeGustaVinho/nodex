// The same logic as operations.cs: every line must draw the same operation nodes.
export function operations(a: number, b: number, name: string, items: number[]): string {
  const sum = a + b * 2;
  const big = Math.max(a, b) >= 10 && !(a === b);
  const missing = name === null;
  let text = name.trim().toUpperCase();
  if (text.startsWith("X") || text.includes("Y")) {
    text = text.replace("X", "Z");
  }
  const parts = text.split(",");
  const joined = parts.join(";");
  const count = items.length + parts.length;
  const parsed = parseInt(joined);
  const root = Math.sqrt(Math.abs(a - b)) % 3;
  let i = 0;
  i++;
  i += count;
  items.push(i);
  console.log("Total: " + sum);
  return parsed.toString();
}
