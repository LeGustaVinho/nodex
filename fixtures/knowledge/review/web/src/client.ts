import axios from "axios";

const api = axios.create({ baseURL: "/" });

export function loadWidget(id: string) {
  return api.get(`/api/widgets/${id}`);
}

export function list() {
  return api.get("/api/gadgets", { params: { all: true } });
}
