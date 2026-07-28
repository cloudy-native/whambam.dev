# Website CDK (TypeScript)

AWS CDK app for hosting the static whambam.dev site.

This package is part of the **website pnpm workspace** (`website/pnpm-workspace.yaml`).

## Setup

From the `website/` directory:

```bash
pnpm install
```

Or only this package:

```bash
cd website/cdk
pnpm install
```

## Useful commands

| Command | Description |
|---------|-------------|
| `pnpm run build` | Compile TypeScript to JS |
| `pnpm run watch` | Watch and recompile |
| `pnpm run test` | Jest unit tests |
| `pnpm exec cdk deploy` | Deploy to default AWS account/region |
| `pnpm exec cdk diff` | Diff against deployed stack |
| `pnpm exec cdk synth` | Emit CloudFormation template |

From the website root you can also use:

```bash
pnpm cdk:build
pnpm cdk:test
pnpm cdk:synth
pnpm cdk:diff
pnpm cdk:deploy
```
