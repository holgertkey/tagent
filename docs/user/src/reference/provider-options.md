# Provider options

Every option of every provider kind, as the apps know them. Set them in a profile:
`[provider_options.<name>]` in `tagent-cli.toml`, or **Options…** in Settings > Providers
of `tagent-gui`. Every value is a string, numbers too. See
[How providers work](../providers/how-providers-work.md).

- **Required** options must be set, or the provider can't be used (`tagent-cli` names the
  missing one; `tagent-gui` marks the profile with ⚠).
- **Secret** options are masked wherever the apps show them, and are best kept in an
  environment variable: see [API keys and environment variables](../providers/api-keys.md).
- Every kind also takes `type`, which says which kind a profile of your own is.

This page is generated from the provider registry.

{{#include generated/provider-options.md}}
