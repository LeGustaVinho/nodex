import { Router } from "express";
import { requireAuth } from "../auth";

const router = Router();
router.use(requireAuth);

router.get("/", (req, res) => {
  res.json([]);
});

router.get("/:id", (req, res) => {
  res.json({ id: req.params.id });
});

export default router;
