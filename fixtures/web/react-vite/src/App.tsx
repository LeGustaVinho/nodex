import { useState } from "react";
import { total } from "./cart";

/** A counter and a small cart: clicking runs code Nodex shows live. */
export function App() {
  const [count, setCount] = useState(0);
  const [items, setItems] = useState<number[]>([]);

  function add() {
    const next = count + 1;
    setCount(next);
    setItems([...items, next * 10]);
  }

  return (
    <main>
      <h1>Count: {count}</h1>
      <button id="add" onClick={add}>
        Add
      </button>
      <p id="total">Total: {total(items)}</p>
    </main>
  );
}
