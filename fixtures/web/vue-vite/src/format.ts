/** "1 click", "2 clicks". */
export function label(n: number): string {
  if (n === 1) {
    return "1 click";
  }
  return `${n} clicks`;
}
