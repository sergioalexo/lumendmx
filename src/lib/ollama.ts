import { fetch } from "@tauri-apps/plugin-http";

export interface OllamaModel {
  name: string;
  size: number;
  digest: string;
  modified_at: string;
}

export interface PullProgress {
  status: string;
  digest?: string;
  total?: number;
  completed?: number;
}

export async function isOllamaReachable(baseUrl: string): Promise<boolean> {
  try {
    const res = await fetch(`${baseUrl}/api/tags`, { method: "GET" });
    return res.ok;
  } catch {
    return false;
  }
}

export async function listModels(baseUrl: string): Promise<OllamaModel[]> {
  const res = await fetch(`${baseUrl}/api/tags`, { method: "GET" });
  if (!res.ok) throw new Error(`Ollama /api/tags failed (${res.status})`);
  const data = (await res.json()) as { models?: OllamaModel[] };
  return data.models ?? [];
}

/** Pulls (installs or updates) a model, streaming NDJSON progress events. Ollama
 * re-pulling an already-installed model tag is how you "update" it -- it fetches
 * only the changed manifest layers, same idea as `docker pull`. */
export async function pullModel(
  baseUrl: string,
  model: string,
  onProgress: (progress: PullProgress) => void,
): Promise<void> {
  const res = await fetch(`${baseUrl}/api/pull`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ name: model, stream: true }),
  });

  if (!res.ok || !res.body) {
    throw new Error(`Ollama /api/pull failed (${res.status}): ${await res.text()}`);
  }

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";

  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });

    let newlineIdx: number;
    while ((newlineIdx = buffer.indexOf("\n")) >= 0) {
      const line = buffer.slice(0, newlineIdx).trim();
      buffer = buffer.slice(newlineIdx + 1);
      if (!line) continue;
      const progress = JSON.parse(line) as PullProgress;
      onProgress(progress);
      if (progress.status === "error") {
        throw new Error(`Ollama pull error for "${model}"`);
      }
    }
  }
}
