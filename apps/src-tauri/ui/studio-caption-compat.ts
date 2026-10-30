// HyperFrames 0.8.181 caption autosave omits the server's optimistic-write headers.
// Reuse the version actually read by this editor; conflicts stay blocked, never retried.
type KnownCaptionVersion = {missing?: boolean, version?: string};

class StudioCaptionCompatController {
  baseFetch: typeof fetch;
  pattern = /^\/api\/projects\/[^/]+\/files\/caption-overrides\.json$/;
  versions = new Map<string, KnownCaptionVersion>();

  constructor() {
    this.baseFetch = globalThis.fetch.bind(globalThis);
    globalThis.fetch = (input: RequestInfo | URL, init?: RequestInit) => this.intercept(input, init);
  }

  captionScope(input: RequestInfo | URL): URL | null {
    const url = new URL(typeof input === "string" || input instanceof URL ? input : input.url, location.href);
    return url.origin === location.origin && this.pattern.test(url.pathname) ? url : null;
  }

  writeIdentity(input: RequestInfo | URL, init?: RequestInit): {method: string, headers: Headers} {
    const asRequest = typeof input === "object" ? (input as Request) : undefined;
    const method = (init?.method || asRequest?.method || "GET").toUpperCase();
    return {method, headers: new Headers(init?.headers || asRequest?.headers)};
  }

  addConditional(headers: Headers, method: string, known: KnownCaptionVersion | undefined): void {
    if (method !== "PUT" || !known || headers.has("If-Match") || headers.has("If-None-Match")) return;
    headers.set(known.missing ? "If-None-Match" : "If-Match", known.missing ? "*" : String(known.version));
  }

  rememberVersion(url: URL, method: string, response: Response): void {
    if (method === "GET" && response.status === 404) this.versions.set(url.href, {missing: true});
    else if ((method === "GET" || method === "PUT") && response.ok) {
      const version = response.headers.get("ETag");
      if (version) this.versions.set(url.href, {version});
    }
  }

  async intercept(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
    const url = this.captionScope(input);
    if (!url) return this.baseFetch(input, init);
    const {method, headers} = this.writeIdentity(input, init);
    this.addConditional(headers, method, this.versions.get(url.href));
    const response = await this.baseFetch(input, {...init, headers});
    this.rememberVersion(url, method, response);
    return response;
  }
}

new StudioCaptionCompatController();
