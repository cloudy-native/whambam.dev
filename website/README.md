# whambam.dev website

Static marketing and documentation site built with [Astro](https://astro.build) and Tailwind CSS.

## Pages

| Route | Content |
|-------|---------|
| `/` | Landing / install snippet |
| `/docs` | User docs (`src/content/docs.md`) |
| `/technology` | Architecture & benchmarks (`src/content/technology.md`) |

## Package manager

This tree uses **[pnpm](https://pnpm.io)** (workspace). Requires pnpm 11+ (see `packageManager` in `package.json`).

```bash
# enable via corepack (recommended) or: brew install pnpm
corepack enable
cd website
pnpm install
```

Workspace packages:

| Path | Package |
|------|---------|
| `.` | Astro site (`whambam-website`) |
| `cdk/` | AWS CDK stack (`cdk`) |

Do not use `npm install` here — lockfile is `pnpm-lock.yaml` only.

## Development

```bash
cd website
pnpm install
pnpm dev       # http://localhost:4321
```

## Build

```bash
pnpm build     # output in dist/
pnpm preview   # serve production build
```

## CDK (optional)

```bash
pnpm cdk:build
pnpm cdk:synth
pnpm cdk:deploy
```

See [cdk/README.md](./cdk/README.md) for details.

## Content

Edit Markdown under `src/content/`. GFM (tables, etc.) is enabled via `remark-gfm`.

## Deploy

`dist/` is a static site (suitable for Vercel, S3, etc.). AWS hosting can use the `cdk/` package.
