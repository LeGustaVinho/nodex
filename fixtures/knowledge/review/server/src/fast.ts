import Fastify from "fastify";

const fastify = Fastify();

async function health() {
  return { ok: true };
}

fastify.get("/api/fast", { schema: {}, handler: health });
