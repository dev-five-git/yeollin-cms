export const DEMO_MODE = import.meta.env.VITE_YEOLLIN_DEMO === 'true'

const DEMO_BASE_PATH = import.meta.env.VITE_YEOLLIN_BASE_PATH ?? ''

export function demoPageHref(path: string): string {
  if (!DEMO_MODE) return path
  if (path === '/') return `${DEMO_BASE_PATH}/`

  return `${DEMO_BASE_PATH}${path}.html`
}

export function normalizeDemoPath(pathname: string): string {
  if (!DEMO_MODE) return pathname

  const withoutBase = pathname.startsWith(DEMO_BASE_PATH)
    ? pathname.slice(DEMO_BASE_PATH.length)
    : pathname
  const withoutHtml = withoutBase.replace(/\.html$/, '')

  return withoutHtml || '/'
}
