import express from "express";
import { readFileSync } from "node:fs";
import { spawn } from "node:child_process";

const app = express();
const users = JSON.parse(readFileSync(new URL("./users.json", import.meta.url), "utf8")) as { id: number; name: string }[];

app.use((req, _res, next) => {
  console.log(`${req.method} ${req.url}`);
  next();
});

app.get("/users", (_req, res) => {
  res.json(users);
});

app.get("/users/:id", (req, res) => {
  const user = users.find((u) => u.id === Number(req.params.id));
  if (!user) {
    res.status(404).json({ error: "not found" });
    return;
  }
  res.json(user);
});

app.post("/backup", (_req, res) => {
  const child = spawn("git", ["status"]);
  child.on("exit", (code) => {
    res.json({ code });
  });
});

const server = app.listen(3000, () => {
  console.log("listening on 3000");
});

process.on("SIGINT", () => {
  server.close();
  process.exit(0);
});
