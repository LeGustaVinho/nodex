export function sumOfEvenSquares(values: number[]): number {
  return values
    .filter((value) => value % 2 === 0)
    .map((value) => value * value)
    .reduce((total, value) => total + value, 0);
}
