# WebScript VS Code Extension

This package provides VS Code syntax highlighting for WebScript `.web` files.

The extension currently contributes:

- the `webscript` language id for `.web` files
- a TextMate grammar at `syntaxes/webscript.tmLanguage.json`
- basic editor behavior through `language-configuration.json`

It is intentionally structured as a normal VS Code extension package so future formatter, linter, language server, and semantic token support can be added without changing the package shape.

## Development

Open this directory in VS Code, press `F5`, and open a `.web` file in the Extension Development Host.

To run the extension against this repository directly:

```bash
code --extensionDevelopmentPath=/home/kristianc/Projects/webscript/extensions/vscode /home/kristianc/Projects/webscript
```

After changing `syntaxes/webscript.tmLanguage.json`, reload the Extension Development Host with `Developer: Reload Window`. VS Code caches TextMate grammars for the active window, so reopening a `.web` file is not always enough.

```bash
npm install
npm run test:grammar
npm run package
```

`npm run test:grammar` is dependency-free and checks that the grammar is valid JSON, contains the expected repository sections, and recognizes representative WebScript fixture snippets.

## Grammar Notes

WebScript uses the same `.web` extension for pages, components, layouts, models, and schemas. When highlighting breaks in one file but not another, check which top-level directive starts the file and add a focused fixture under `test/fixtures`.

Important cases to keep covered:

- `@component Name {}` single-line component declarations
- `@client` blocks with nested `fn name() { ... }` bodies
- `@style` blocks with nested CSS such as `@media`
- `@model` fields with decorators like `@primary`, `@auto`, `@default(...)`, `@references(...)`, and `@index(...)`
- `@schema` fields with decorators like `@optional`, `@min(...)`, `@max(...)`, and `@email`

Top-level block grammars intentionally close on an unindented `}`. This keeps nested `}` lines inside client functions, server blocks, and CSS from ending the outer directive too early. If the language starts allowing indented top-level closing braces, revisit those TextMate `end` patterns and add fixtures first.
