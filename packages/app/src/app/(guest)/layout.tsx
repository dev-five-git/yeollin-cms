export default function GuestLayout({
  children,
}: {
  children: React.ReactNode
}) {
  // Guest routes are only accessible when NOT logged in
  // Redirect logic is handled by Rust auth middleware
  return <>{children}</>
}
