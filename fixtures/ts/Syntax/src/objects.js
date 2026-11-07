const api = {
  async get(url) { return fetch(url); },
  post: (url, body) => fetch(url, { method: "POST", body }),
  nested: { deep() { return 1; }, value: 2 },
  plain: 3,
};
module.exports = { run() { api.get("/x"); }, helper: function () { return 2; } };
export default { name: "comp", methods: { click() { this.count++; } }, data() { return { count: 0 }; } };
