import { useState } from "react";

type Status = "idle" | "uploading" | "done" | "error";

/** A React state variable of string literals. */
export function useUpload() {
  const [status, setStatus] = useState<Status>("idle");

  async function upload(file: File) {
    if (status === "uploading") {
      return;
    }
    setStatus("uploading");
    try {
      await fetch("/api/upload", { method: "POST", body: file });
      setStatus("done");
    } catch {
      setStatus("error");
    }
  }

  function reset() {
    setStatus("idle");
  }

  return { status, upload, reset };
}
