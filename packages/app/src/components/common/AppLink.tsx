'use client'

import { css } from '@devup-ui/react'
import Link from 'next/link'
import type { ReactNode } from 'react'

interface AppLinkProps {
  children: ReactNode
  href: string
}

export function AppLink({ children, href }: AppLinkProps) {
  return (
    <Link className={css({ textDecoration: 'none' })} href={href}>
      {children}
    </Link>
  )
}
