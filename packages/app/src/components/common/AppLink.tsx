'use client'

import Link from 'next/link'
import type { ReactNode } from 'react'

import { DEMO_MODE, demoPageHref } from '@/lib/demo-path'

interface AppLinkProps {
  children: ReactNode
  href: string
}

export function AppLink({ children, href }: AppLinkProps) {
  const style = { textDecoration: 'none' }

  if (DEMO_MODE) {
    return (
      <a href={demoPageHref(href)} style={style}>
        {children}
      </a>
    )
  }

  return (
    <Link href={href} style={style}>
      {children}
    </Link>
  )
}
