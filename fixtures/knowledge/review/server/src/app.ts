import express from "express";
import * as widgets from "./widgets.controller";

const app = express();
app.get("/api/catalog", widgets.list);
app.listen(3000);
