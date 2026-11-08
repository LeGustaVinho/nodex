import { BrowserRouter, Route, Routes } from "react-router-dom";

export function OrdersPage() {
  async function saveOrder() {
    await fetch("/api/orders", { method: "POST" });
  }

  return (
    <div>
      <button onClick={saveOrder}>Save order</button>
      <button onClick={() => window.print()}>Print</button>
    </div>
  );
}

export function SettingsPage() {
  return <div>Settings</div>;
}

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/orders" element={<OrdersPage />} />
        <Route path="/settings" element={<SettingsPage />} />
      </Routes>
    </BrowserRouter>
  );
}
