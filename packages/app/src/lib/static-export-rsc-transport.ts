export const STATIC_EXPORT_DEPLOYMENT_ID = 'yeollin-cms-pages-demo'

/**
 * Adapts vinext RSC requests to artifacts emitted by `output: 'export'` on a
 * plain static host such as GitHub Pages.
 */
export function installStaticExportRscTransport(
  deploymentId: string,
  basePath: string,
): void {
  const nativeFetch = globalThis.fetch.bind(globalThis)
  const normalizedBasePath = basePath.replace(/\/$/u, '')

  globalThis.fetch = async (input, init) => {
    const request = new Request(input, init)
    if (request.method !== 'GET' || request.headers.get('RSC') !== '1') {
      return nativeFetch(input, init)
    }

    const visibleUrl = new URL(request.url)
    if (
      visibleUrl.origin !== globalThis.location.origin ||
      (normalizedBasePath !== '' &&
        visibleUrl.pathname !== normalizedBasePath &&
        !visibleUrl.pathname.startsWith(`${normalizedBasePath}/`))
    ) {
      return nativeFetch(input, init)
    }

    const artifactUrl = new URL(visibleUrl)
    if (!artifactUrl.pathname.endsWith('.rsc')) {
      const rootPath =
        normalizedBasePath === '' ? '/' : `${normalizedBasePath}/`
      artifactUrl.pathname =
        artifactUrl.pathname === rootPath
          ? `${rootPath}index.rsc`
          : `${artifactUrl.pathname.replace(/\/$/u, '')}.rsc`
    }
    artifactUrl.searchParams.delete('_rsc')

    const artifactResponse = await nativeFetch(artifactUrl, {
      credentials: request.credentials,
      headers: request.headers,
      signal: request.signal,
    })
    if (!artifactResponse.ok) {
      await artifactResponse.body?.cancel()
      return nativeFetch(input, init)
    }

    const headers = new Headers(artifactResponse.headers)
    headers.set('Content-Type', 'text/x-component')
    headers.set('X-Vinext-RSC-Compatibility-Id', deploymentId)

    const response = new Response(artifactResponse.body, {
      headers,
      status: artifactResponse.status,
      statusText: artifactResponse.statusText,
    })

    // The `.rsc` artifact is a transport detail, not a redirect destination.
    Object.defineProperty(response, 'url', { value: visibleUrl.href })
    return response
  }
}
