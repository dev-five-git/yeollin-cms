/**
 * Yeollin CMS Type Definitions
 */

/**
 * Route configuration for individual pages
 * Export this from route.ts in each folder
 */
export interface RouteConfig {
  /** Display label in menu */
  label: string
  /** Icon name (optional) */
  icon?: string
  /** Sort order (lower = higher priority) */
  order?: number
  /** Hide from menu but keep route accessible */
  hidden?: boolean
}

/**
 * Plugin-level route configuration
 * Export this from the plugin's root route.ts
 */
export interface PluginRouteConfig extends RouteConfig {
  /** Plugin identifier (used in URL path) */
  name: string
}

/**
 * Parsed menu item (generated from folder structure)
 */
export interface MenuItem {
  id: string
  label: string
  icon?: string
  path: string
  order: number
  children?: MenuItem[]
}

/**
 * Plugin menu configuration (auto-generated)
 */
export interface PluginMenu {
  plugin: string
  items: MenuItem[]
}
