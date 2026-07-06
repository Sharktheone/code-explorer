# Code Explorer

Code Explorer is a Rust desktop app for indexing a local code directory or public HTTPS Git repository and visualizing where code volume lives.

## Features

- Slint desktop UI.
- Index a directory passed on the command line, for example `code-explorer .`.
- Open a folder from the app.
- Clone public HTTPS repositories into an app cache and scan them.
- Top GitHub-style language composition bar using generated GitHub Linguist colors.
- Directory list with prominent-language dots, attention colors, metrics, percentages, and optional language mini-bars.
- LOC-sized boxes grid with a slider for how many subdirectories to show.
- Filters for extensions, languages, directories, `.gitignore`, hidden files, and max depth.
- Configurable metric: total LOC, code LOC, or file count.

## Language Definitions

The local `languages.yml` file is treated as source data from GitHub Linguist and is ignored by Git. Generate the compact app-owned asset with:

```bash
cargo run -p convert-linguist-languages
```

The generated `assets/languages.generated.json` file is tracked.

## Development

```bash
cargo run -p convert-linguist-languages
cargo run -p code-explorer -- .
cargo test
```

## Scope

The first version supports public HTTPS clone URLs only. SSH, private repositories, credential storage, and token prompts are intentionally out of scope.
