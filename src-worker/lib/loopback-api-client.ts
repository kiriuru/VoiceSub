import { withLoopbackAuth } from "../../src/lib/loopback-api";

export async function apiFetch(url: string, init?: RequestInit): Promise<Response> {
  return fetch(url, withLoopbackAuth(init));
}
