// A CommonJS helper, as found in older Node code.
const util = require("node:util");

function money(amount) {
  if (typeof amount !== "number") {
    return "?";
  }
  return util.format("$%s", amount.toFixed(2));
}

module.exports = { money };
