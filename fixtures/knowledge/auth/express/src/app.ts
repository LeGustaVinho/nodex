import express from "express";
import ordersRouter from "./routes/orders";
import { requireRole } from "./auth";

const app = express();
const adminRouter = express.Router();

adminRouter.delete("/cache", requireRole("admin"), (req, res) => {
  res.sendStatus(204);
});

app.use("/api/orders", ordersRouter);
app.use("/admin", adminRouter);
app.listen(3000);
