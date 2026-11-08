import { Command } from "commander";
import cron from "node-cron";
import { Kafka } from "kafkajs";

const program = new Command();

async function addItem(name: string) {
  console.log("adding", name);
}

program.command("add <name>").description("Adds an item").action(addItem);

program
  .command("list")
  .action(() => {
    console.log("listing");
  });

function heartbeat() {
  console.log("alive");
}

setInterval(heartbeat, 5000);

cron.schedule("0 3 * * *", () => {
  console.log("nightly cleanup");
});

const consumer = new Kafka({ brokers: ["localhost:9092"] }).consumer({ groupId: "orders" });

async function handleOrder({ message }: any) {
  console.log(message.value);
}

consumer.run({ eachMessage: handleOrder });

export async function listen(channel: any) {
  channel.consume("invoices", async (msg: any) => {
    console.log(msg);
  });
}

program.parse();
