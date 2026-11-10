export function loadAll(values: number[]): Promise<number[]> {
  return Promise.all(values.map(async (value) => value * value))
    .then((squares) => squares)
    .catch(() => []);
}
