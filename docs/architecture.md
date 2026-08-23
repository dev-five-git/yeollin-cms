# Yeollin CMS Architecture

Rust + Next.js plugin-based CMS framework. Plugins bundle API routes and frontend
pages in a single crate; `yeollin-cli` assembles them into one binary that serves
both the API and the statically exported frontend.

```mermaid
flowchart TB
    subgraph CLI["🔧 yeollin-cli"]
        direction LR
        init["init"]
        prebuild["prebuild"]
        dev["dev"]
        build["build"]
    end

    subgraph CoreCrates["🦀 Rust Core Crates"]
        direction TB
        core["yeollin-core<br/>(ContentMeta, MenuItem, MenuConfig)"]
        auth["yeollin-auth<br/>(JWT, Argon2, Middleware)"]
        pluginLib["yeollin-plugin<br/>(PluginMetadata, FrontendAssets)"]
        macros["yeollin-plugin-macros<br/>(yeollin_plugin!, yeollin_app!)"]
        appLib["yeollin-app<br/>(YeollinApp, YeollinAppBuilder)"]
    end

    subgraph Plugins["🧩 Plugins (Rust Crate + Next.js)"]
        direction TB
        subgraph P1["example-plugin"]
            p1routes["API Routes<br/>/api/example/*"]
            p1fe["Frontend Pages<br/>app/(group)/page.tsx"]
        end
        subgraph P2["example-memo-plugin"]
            p2routes["API Routes<br/>/memo/*"]
            p2fe["Frontend Pages"]
            p2model["SeaORM Models"]
            p2migrate["Vespertide Migrations"]
        end
    end

    subgraph Frontend["⚛️ Frontend (packages/app)"]
        direction TB
        nextjs["Next.js 16 + React 19"]
        devupui["@devup-ui/react"]
        devupapi["@devup-api/fetch"]
        rq["@tanstack/react-query"]
    end

    subgraph Runtime["🚀 Runtime (Single Binary)"]
        direction TB
        axum["Axum Router<br/>(port 3001)"]
        subgraph Services["Services"]
            direction LR
            jwtS["JWT Auth"]
            staticS["Static File Server"]
            openapiS["OpenAPI (Vespera)"]
            dbS["SQLite + SeaORM"]
            menuS["Menu Registry"]
        end
    end

    subgraph BuildOutput["📦 Build Output"]
        direction LR
        dotYeollin[".yeollin/app/<br/>(assembled frontend)"]
        binary["Single Binary<br/>(API + Static + DB)"]
    end

    %% Core dependencies
    core --> auth
    core --> pluginLib
    macros --> pluginLib
    auth --> appLib
    pluginLib --> appLib

    %% Plugin registration
    P1 -->|"PluginMetadata"| appLib
    P2 -->|"PluginMetadata"| appLib

    %% CLI flow
    prebuild -->|"assemble plugins frontend"| dotYeollin
    dotYeollin -->|"next build + export"| binary
    appLib -->|"cargo build --release"| binary

    %% Frontend composition
    devupui --> nextjs
    devupapi --> nextjs
    rq --> nextjs
    nextjs --> dotYeollin

    %% Runtime composition
    appLib --> axum
    axum --> Services

    %% Dev mode
    dev -->|"single port :3001, proxies to Next :3000"| axum

    classDef rust fill:#b7410e,stroke:#ff6633,color:#fff
    classDef node fill:#215732,stroke:#3fb950,color:#fff
    classDef cli fill:#1a3a5c,stroke:#58a6ff,color:#fff
    classDef output fill:#4a2a6b,stroke:#bc8cff,color:#fff
    classDef runtime fill:#5c3a1a,stroke:#d29922,color:#fff

    class core,auth,pluginLib,macros,appLib rust
    class nextjs,devupui,devupapi,rq node
    class init,prebuild,dev,build cli
    class dotYeollin,binary output
    class axum,jwtS,staticS,openapiS,dbS,menuS runtime
```

## Build flow

1. `cargo build` — produces a binary that can export plugin/menu metadata via
   `YEOLLIN_EXPORT_PLUGINS` / `YEOLLIN_EXPORT_MENUS`.
2. `yeollin prebuild` — extracts the `packages/app` template into `.yeollin/app/`,
   copies each plugin's `app/` pages in, and writes `menus.json` / `plugins.json`.
3. `next build` — static export to `.yeollin/app/out/`.
4. `cargo build --release` — final binary, embedding static files via `include_dir!`.

## Dev mode

`yeollin dev` serves everything on a single port (3001). The Axum router handles
API routes and proxies everything else to the internal Next.js dev server on 3000,
including the Turbopack HMR WebSocket at `/_next/hmr`.
