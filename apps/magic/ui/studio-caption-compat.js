class StudioCaptionCompatController {
  baseFetch;
  pattern = /^\/api\/projects\/[^/]+\/files\/caption-overrides\.json$/;
  versions = new Map;
  constructor() {
    this.baseFetch = globalThis.fetch.bind(globalThis);
    globalThis.fetch = (input, init) => this.intercept(input, init);
  }
  captionScope(input) {
    const url = new URL(typeof input === "string" || input instanceof URL ? input : input.url, location.href);
    return url.origin === location.origin && this.pattern.test(url.pathname) ? url : null;
  }
  writeIdentity(input, init) {
    const asRequest = typeof input === "object" ? input : undefined;
    const method = (init?.method || asRequest?.method || "GET").toUpperCase();
    return { method, headers: new Headers(init?.headers || asRequest?.headers) };
  }
  addConditional(headers, method, known) {
    if (method !== "PUT" || !known || headers.has("If-Match") || headers.has("If-None-Match"))
      return;
    headers.set(known.missing ? "If-None-Match" : "If-Match", known.missing ? "*" : String(known.version));
  }
  rememberVersion(url, method, response) {
    if (method === "GET" && response.status === 404)
      this.versions.set(url.href, { missing: true });
    else if ((method === "GET" || method === "PUT") && response.ok) {
      const version = response.headers.get("ETag");
      if (version)
        this.versions.set(url.href, { version });
    }
  }
  async intercept(input, init) {
    const url = this.captionScope(input);
    if (!url)
      return this.baseFetch(input, init);
    const { method, headers } = this.writeIdentity(input, init);
    this.addConditional(headers, method, this.versions.get(url.href));
    const response = await this.baseFetch(input, { ...init, headers });
    this.rememberVersion(url, method, response);
    return response;
  }
}
new StudioCaptionCompatController;
