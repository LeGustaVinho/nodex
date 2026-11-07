import { useEffect, useState, type ReactNode } from "react";

type ButtonProps = { label: string; onClick?: () => void; children?: ReactNode };

export function Button({ label, onClick, children }: ButtonProps) {
  return (
    <button className="btn" onClick={onClick} disabled={!onClick}>
      {label}
      {children}
    </button>
  );
}

export default function App({ items }: { items: string[] }) {
  const [count, setCount] = useState(0);
  const [query, setQuery] = useState("");

  useEffect(() => {
    document.title = `Count: ${count}`;
  }, [count]);

  const handleClick = () => {
    setCount((c) => c + 1);
  };

  const visible = items.filter((i) => i.includes(query)).map((i) => <li key={i}>{i}</li>);

  if (items.length === 0) {
    return <p>No items</p>;
  }

  return (
    <>
      <input value={query} onChange={(e) => setQuery(e.target.value)} />
      <Button label={`Clicked ${count} times`} onClick={handleClick}>
        <span aria-hidden />
      </Button>
      <ul>{visible}</ul>
      {count > 3 && <strong>Many clicks</strong>}
      <footer>&copy; Nodex&nbsp;2026</footer>
    </>
  );
}
