# Yeollin CMS

A modern, type-safe CMS framework inspired by [Tauri](https://tauri.app)'s architecture philosophy.

**Yeollin** (열린, Korean for "open") embodies our vision: an open, extensible CMS that bridges the gap between powerful backend capabilities and delightful frontend experiences.

## Philosophy

Like Tauri revolutionized desktop app development by combining web frontends with Rust backends, Yeollin CMS brings this same architectural clarity to content management:

- **Frontend/Backend Separation**: Clean boundaries between presentation and data layers
- **Type Safety End-to-End**: From database schema to API to UI components
- **Zero Runtime Overhead**: Build-time optimizations wherever possible
- **Developer Experience First**: FastAPI-like ergonomics for Rust, CSS-in-JS without runtime cost

## Tech Stack

### Frontend

| Technology | Purpose |
|------------|---------|
| [Next.js](https://nextjs.org) | React framework with App Router |
| [@devup-ui/react](https://github.com/user/devup-ui) | Zero-runtime CSS-in-JS components |
| [@devup-api/fetch](https://github.com/user/devup-api) | Type-safe API client from OpenAPI |

### Backend

| Technology | Purpose |
|------------|---------|
| [Vespera](https://github.com/dev-five-git/vespera) | Rust/Axum API framework with FastAPI-like DX |
| [Vespertide](https://github.com/dev-five-git/vespertide) | Declarative database schema management |
| [sea-orm](https://www.sea-ql.org/SeaORM/) | Async ORM for Rust |

## Core Components

### Devup UI

Build-time CSS extraction with zero runtime JavaScript for styling.

```tsx
// Write intuitive style props
<Box bg="$primary" p={4} _hover={{ bg: "$primaryHover" }}>
  <Text typography="heading">Hello Yeollin</Text>
</Box>

// Compiles to static CSS classes at build time
<div class="a b c">
  <span class="d">Hello Yeollin</span>
</div>
```

Key features:
- Responsive arrays: `p={[2, null, 4, null, 6]}` (mobile → tablet → PC)
- Pseudo-selectors: `_hover`, `_focus`, `_dark`
- Theme tokens: `$primary`, `$background`
- CSS extraction at compile time

### Devup API

Type-safe API client auto-generated from OpenAPI schemas.

```tsx
import { createApi, type DevupObject } from '@devup-api/fetch'

const api = createApi('https://api.yeollin.dev')

// Fully typed request and response
const user = await api.get('/users/{id}', { params: { id: '123' } })

// Use schema types directly
function UserCard({ user }: { user: DevupObject['User'] }) {
  return <Box>{user.name}</Box>
}
```

Ecosystem:
- `@devup-api/react-query` - React Query hooks
- `@devup-api/zod` - Runtime validation schemas
- `@devup-api/hookform` - React Hook Form integration
- `@devup-api/ui` - Auto-generated CRUD components

### Vespera

FastAPI-like developer experience for Rust APIs.

```rust
// Route handlers with automatic OpenAPI generation
#[vespera::route(get, path = "/{id}", tags = ["users"])]
pub async fn get_user(Path(id): Path<u32>) -> Json<User> {
    // ...
}

// Schema derivation
#[derive(Serialize, Deserialize, vespera::Schema)]
pub struct User {
    id: u32,
    name: String,
}
```

Features:
- Zero-config OpenAPI 3.1 generation
- Compile-time route discovery
- Swagger UI and ReDoc built-in
- Serde attribute support

### Vespertide

Declarative database schema definition with migration generation.

```json
{
  "$schema": "https://raw.githubusercontent.com/dev-five-git/vespertide/refs/heads/main/schemas/model.schema.json",
  "name": "article",
  "columns": [
    { "name": "id", "type": "integer", "nullable": false, "primary_key": { "auto_increment": true } },
    { "name": "title", "type": "text", "nullable": false },
    { 
      "name": "status", 
      "type": { 
        "kind": "enum", 
        "name": "article_status", 
        "values": ["draft", "review", "published"] 
      }, 
      "nullable": false, 
      "default": "'draft'" 
    },
    { "name": "created_at", "type": "timestamptz", "nullable": false, "default": "NOW()" }
  ]
}
```

Features:
- JSON schema validation in IDE
- Automatic migration generation
- SeaORM entity export
- Cross-database support (PostgreSQL, MySQL, SQLite)

## Architecture

```
yeollin-cms/
├── crates/
│   ├── core/                # Shared structs and types
│   ├── app/                 # Backend CMS application
│   └── plugin/              # Plugin interface traits
├── packages/
│   └── app/                 # Next.js frontend with devup-ui
└── plugins/
    └── example-plugin/
        ├── api/             # Rust backend extension
        └── app/             # Frontend UI (menus, pages)
```

## Plugin System

Yeollin CMS features a **unified plugin architecture** where a single crate contains both backend logic and frontend UI.

### How It Works

```
┌─────────────────────────────────────────────────────────────┐
│                     Build Process                            │
├─────────────────────────────────────────────────────────────┤
│  1. Read plugin metadata from crates/app/src/main.rs        │
│  2. Scan each plugin's api/ for Rust routes                 │
│  3. Scan each plugin's app/ for TSX components              │
│  4. Merge routes into OpenAPI spec                          │
│  5. Inject menus and UI into packages/app                   │
└─────────────────────────────────────────────────────────────┘
```

### Plugin Structure

```
plugins/my-plugin/
├── Cargo.toml               # Rust crate manifest
├── api/
│   ├── src/
│   │   ├── lib.rs           # Plugin entry point
│   │   └── routes/          # Vespera route handlers
│   └── models/              # Vespertide schemas
└── app/
    ├── menu.json            # Menu configuration
    └── pages/               # TSX pages and components
        └── dashboard.tsx
```

### Plugin Metadata

Every plugin exposes metadata containing an `axum::Router` for route merging:

```rust
// plugins/my-plugin/api/src/lib.rs
use yeollin_plugin::PluginMetadata;
use axum::Router;

pub fn metadata() -> PluginMetadata {
    PluginMetadata {
        name: "my-plugin",
        version: "0.1.0",
        router: router(),                     // axum::Router to merge
        frontend_assets: include_dir!("../app"),  // Embedded TSX
    }
}

fn router() -> Router {
    Router::new()
        .route("/my-plugin/items", get(list_items))
        .route("/my-plugin/items/:id", get(get_item))
}
```

### Usage Patterns

Yeollin supports two development patterns, similar to Tauri's unified build approach:

#### Pattern 1: Consumer (main.rs only)

Install and use published plugins as dependencies:

```toml
# crates/app/Cargo.toml
[dependencies]
yeollin-plugin-blog = "0.1"
yeollin-plugin-media = "0.2"
```

```rust
// crates/app/src/main.rs
use yeollin::register_plugin;

fn main() {
    let app = yeollin::app()
        .register_plugin(yeollin_plugin_blog::metadata())
        .register_plugin(yeollin_plugin_media::metadata())
        .build();
    
    app.run();
}
```

#### Pattern 2: Developer (main.rs + lib.rs)

Develop custom plugins locally within the monorepo:

```rust
// plugins/my-plugin/api/src/lib.rs
pub fn metadata() -> PluginMetadata { /* ... */ }

// crates/app/src/main.rs
use my_plugin;  // Local workspace dependency

fn main() {
    let app = yeollin::app()
        .register_plugin(my_plugin::metadata())           // Local plugin
        .register_plugin(yeollin_plugin_blog::metadata()) // Published plugin
        .build();
    
    app.run();
}
```

```toml
# crates/app/Cargo.toml
[dependencies]
my-plugin = { path = "../../plugins/my-plugin/api" }  # Local
yeollin-plugin-blog = "0.1"                            # Published
```

### Distribution

Plugins are distributed as **Rust crates** that bundle:
- `axum::Router` for API route merging
- Embedded frontend assets (TSX, menu configs) via `include_dir!`

```toml
# plugins/my-plugin/api/Cargo.toml
[dependencies]
yeollin-plugin = "0.1"
include_dir = "0.7"
axum = "0.7"
```

## Getting Started

```bash
# Clone the repository
git clone https://github.com/your-org/yeollin-cms.git
cd yeollin-cms

# Install dependencies
pnpm install

# Start development
pnpm dev
```

## Inspiration

Yeollin CMS draws inspiration from:

- **[Tauri](https://tauri.app)**: Frontend/backend separation with Rust performance
- **[FastAPI](https://fastapi.tiangolo.com)**: Developer-friendly API design with automatic documentation
- **[Payload CMS](https://payloadcms.com)**: Type-safe, extensible CMS architecture
- **[Strapi](https://strapi.io)**: Headless CMS flexibility

## License

MIT

---

Built with care by [DevFive](https://devfive.co)
