# WebScript VS Code Extension

This package provides VS Code syntax highlighting for WebScript `.web` files.

The extension currently contributes:

- the `webscript` language id for `.web` files
- a TextMate grammar at `syntaxes/webscript.tmLanguage.json`
- basic editor behavior through `language-configuration.json`

It is intentionally structured as a normal VS Code extension package so future formatter, linter, language server, and semantic token support can be added without changing the package shape.

## Development

Open this directory in VS Code, press `F5`, and open a `.web` file in the Extension Development Host.

```bash
npm install
npm run test:grammar
npm run package
```

`npm run test:grammar` is dependency-free and checks that the grammar is valid JSON, contains the expected repository sections, and recognizes representative WebScript fixture snippets.
