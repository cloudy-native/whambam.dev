# whambam.dev website

Static marketing and documentation site built with [Astro](https://astro.build) and Tailwind CSS.

## Pages

| Route | Content |
|-------|---------|
| `/` | Landing / install snippet |
| `/docs` | User docs (`src/content/docs.md`) |
| `/technology` | Architecture & benchmarks (`src/content/technology.md`) |

## Development

```bash
cd website
pnpm install   # or npm install
pnpm dev       # http://localhost:4321
```

## Build

```bash
pnpm build     # output in dist/
pnpm preview   # serve production build
```

## Content

Edit Markdown under `src/content/`. GFM (tables, etc.) is enabled via `remark-gfm`.

## Deploy

`dist/` is a static site (suitable for Vercel, S3, etc.). The optional `cdk/` folder is unchanged for AWS hosting if used.
