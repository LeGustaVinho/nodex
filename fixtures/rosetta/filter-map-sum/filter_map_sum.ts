export function sumOfEvenSquares(numbers: number[]): number {
  let total = 0;
  for (const n of numbers) {
    if (n % 2 === 0) {
      total += n * n;
    }
  }
  return total;
}
