import { initLoopbackApiToken, withLoopbackAuth } from "./loopback-api";

export {
  initLoopbackApiToken,
  loopbackApiHeaders,
  loopbackApiToken,
  LOOPBACK_TOKEN_HEADER,
  withLoopbackAuth,
} from "./loopback-api";

export function apiFetch(url: string, init?: RequestInit): Promise<Response> {
  return fetch(url, withLoopbackAuth(init));
}
