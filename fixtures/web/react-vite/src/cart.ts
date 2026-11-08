/** Sum of the prices, with a discount from 3 items on. */
export function total(prices: number[]): number {
  let sum = 0;
  for (const p of prices) {
    sum += p;
  }
  if (prices.length >= 3) {
    sum = Math.round(sum * 0.9);
  }
  return sum;
}
