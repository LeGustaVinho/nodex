import express from "express";
import { controller } from "legacy-lib";

const app = express();
// `controller` comes from a package: its `ping` is no function of this project.
app.get("/api/ping", controller.ping);
