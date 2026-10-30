// HyperFrames 0.8.181 caption autosave omits the server's optimistic-write headers.
// Reuse the version actually read by this editor; conflicts stay blocked, never retried.
(() => {
  type Known = {missing?: boolean, version?: string};
  const CAPTION_OVERRIDES = /^\/api\/projects\/[^/]+\/files\/caption-overrides\.json$/;
  const fetch = globalThis.fetch.bind(globalThis), versions = new Map<string, Known>();

  // The caption file this editor owns, or null for every other request it must not touch.
  const captionScope = (input: RequestInfo | URL): URL | null => {
    const url = new URL(typeof input === "string" || input instanceof URL ? input : input.url, location.href);
    return url.origin === location.origin && CAPTION_OVERRIDES.test(url.pathname) ? url : null;
  };

  // A Request carries its own method and headers; an init argument takes precedence.
  const writeIdentity = (input: RequestInfo | URL, init?: RequestInit): {method: string, headers: Headers} => {
    const asRequest = typeof input === "object" ? (input as Request) : undefined;
    const method = (init?.method || asRequest?.method || "GET").toUpperCase();
    return {method, headers: new Headers(init?.headers || asRequest?.headers)};
  };

  // Only an unconditional PUT gets the editor's version; a caller's own condition wins.
  const addConditional = (headers: Headers, method: string, known: Known | undefined): void => {
    if (method !== "PUT" || !known || headers.has("If-Match") || headers.has("If-None-Match")) return;
    headers.set(known.missing ? "If-None-Match" : "If-Match", known.missing ? "*" : String(known.version));
  };

  const rememberVersion = (url: URL, method: string, response: Response): void => {
    if (method === "GET" && response.status === 404) versions.set(url.href, {missing: true});
    else if ((method === "GET" || method === "PUT") && response.ok) {
      const version = response.headers.get("ETag");
      if (version) versions.set(url.href, {version});
    }
  };

  globalThis.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = captionScope(input);
    if (!url) return fetch(input, init);
    const {method, headers} = writeIdentity(input, init);
    addConditional(headers, method, versions.get(url.href));
    const response = await fetch(input, {...init, headers});
    rememberVersion(url, method, response);
    return response;
  };
})();
