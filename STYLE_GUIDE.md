# Markdown Style Guide

This style guide outlines the formatting rules for markdown files in this repository. These rules are enforced by markdownlint and are optimized for GitBook compatibility.

## Recommended Workflow

1. Use a [markdownlint extension for your editor of choice][markdownlint-extension].
1. Use a [typos extension for your editor of choice][typos-extension].
1. Run a markdownlint check before pushing code.
1. Run a typos check before pushing code.

Alternatively (and this is recommended at least as additional step) you could ask an integrated LLM to do a check for spelling, grammar and markdown compliance (based on the `.markdownlint.json` file).

## Basic Formatting

- Use 2 spaces for indentation
- No hard tabs allowed
- No trailing spaces at the end of lines
- Use dashes (`-`) for unordered lists
- Maximum line length is not enforced (MD013 is disabled)

## Headers

- First line of the file does not need to be a top-level heading (MD041 is disabled)
- Multiple headings with the same content are allowed (MD024 is disabled)
- Multiple top-level headings are allowed (MD025 is disabled)

## Links

GitBook prefers inline links over reference-style links. However, to manage links and facilitate updating in this repository we urge you to use reference style links.

> Note that there is a Github workflow in the Gitbook synced repository to transform reference style links for content that is synced from source repositories (like `nexus-sdk` and `nexus-next`). This ensures that all links in the `gitbook-docs` repo are inline style links for Gitbook compatibility.

In this repo, use reference-style links at the bottom of the file:

```markdown
...
[Link text][reference]
...

<!-- List of references -->

[reference]: https://example.com
```

over inline links, like:

```markdown
[Link text](https://example.com)
```

## Lists

- Use consistent numbering style (MD029)
- Use dashes (`-`) for unordered lists (MD004)
- Indent list items with 2 spaces (MD007)

## Punctuation

- Avoid using the following punctuation at the end of headings: `.`, `,`, `;`, `:`, `!` (MD026)

## HTML and Special Characters

- HTML tags are allowed (MD033 is disabled)
- Use proper escaping for special characters

## Best Practices

1. Keep content clear and concise
1. Use descriptive link text
1. Maintain consistent formatting throughout the document
1. Use proper heading hierarchy
1. Include alt text for images
1. Use code blocks with appropriate language specification

Remember to run the markdown linter to ensure your content follows these guidelines.

<!-- List of references -->

[markdownlint-extension]: https://github.com/DavidAnson/markdownlint?tab=readme-ov-file#related
[typos-extension]: https://github.com/tekumara/typos-lsp
