import { memo, useCallback, useContext, useEffect, useMemo, useRef, useState, createContext } from "react";

export const ThemeContext = createContext("light");

/** Counts up every second while it is shown. */
export function useTicker(intervalMs: number): number {
  const [ticks, setTicks] = useState(0);
  useEffect(() => {
    const id = setInterval(() => setTicks((t) => t + 1), intervalMs);
    return () => clearInterval(id);
  }, [intervalMs]);
  return ticks;
}

export function Counter({ start, step }: { start: number; step: number }) {
  const [count, setCount] = useState(start);
  const history = useRef<number[]>([]);
  const theme = useContext(ThemeContext);
  const ticks = useTicker(1000);
  const doubled = useMemo(() => count * 2, [count]);

  useEffect(() => {
    history.current.push(count);
    document.title = `Count: ${count}`;
  }, [count]);

  const increment = useCallback(() => {
    if (count >= 100) {
      return;
    }
    setCount((c) => c + step);
  }, [count, step]);

  if (count < 0) {
    return <p className="error">Negative</p>;
  }

  return (
    <div className={theme}>
      <Row label="count" value={count} />
      <Row label="doubled" value={doubled} />
      <button onClick={increment}>+{step}</button>
      <span>{ticks}s</span>
    </div>
  );
}

export const Row = memo(function Row({ label, value }: { label: string; value: number }) {
  return (
    <p>
      {label}: {value}
    </p>
  );
});
