import type * as PrismNamespace from 'prismjs';

const SERVER_KEYWORDS =
  /\b(?:await|return|if|else|while|try|catch|throw|fail|redirect|spawn|timeout|fn|type|import|export|notFound|sleep|fetch)\b/;

const BUILTIN_TYPES =
  /\b(?:string|int|float|bool|date|datetime|duration|bytes|object|void|Redirect|Json|Money|File)\b/;

function createExpressionGrammar(): PrismNamespace.Grammar {
  return {
    comment: [
      {
        pattern: /\/\/[^\n]*/,
        greedy: true,
      },
      {
        pattern: /\/\*[\s\S]*?\*\//,
        greedy: true,
      },
    ],
    string: {
      pattern: /"(?:\\.|[^"\\])*"/,
      greedy: true,
    },
    number: [
      {
        pattern: /\b\d+(?:\.\d+)?(?:ms|s|m|h|d)\b/,
        alias: 'number',
      },
      /\b\d+(?:\.\d+)?\b/,
    ],
    boolean: /\b(?:true|false|null)\b/,
    keyword: SERVER_KEYWORDS,
    'class-name': BUILTIN_TYPES,
    operator: /->|:=|=>|[+\-*/%]=?|&&|\|\||[<>]=?|==|!=|!/,
    punctuation: /[{}[\]();,.:?]/,
    property: {
      pattern: /(?<=[\w)\]])\.[a-z_]\w*/i,
    },
    function: {
      pattern: /(?<=\.)[a-z_]\w*(?=\s*\()/i,
    },
    directive: {
      pattern: /@[a-z][\w-]*(?:\([^)]*\))?/i,
      alias: 'keyword',
    },
    generic: {
      pattern: /<[^>]+>/,
      inside: {
        punctuation: /[<>[\].]/,
        'class-name': /[A-Za-z_]\w*/,
      },
    },
  };
}

export function registerWebLanguage(Prism: typeof PrismNamespace): void {
  const expression = createExpressionGrammar();

  Prism.languages.web = Prism.languages.extend('markup', {
    comment: [
      {
        pattern: /\/\/[^\n]*/,
        greedy: true,
      },
      {
        pattern: /\/\*[\s\S]*?\*\//,
        greedy: true,
      },
    ],
    directive: {
      pattern: /@[a-z][\w-]*(?:\([^)]*\))?/i,
      alias: 'keyword',
    },
    keyword: SERVER_KEYWORDS,
    'class-name': BUILTIN_TYPES,
    string: {
      pattern: /"(?:\\.|[^"\\])*"/,
      greedy: true,
    },
    number: [
      {
        pattern: /\b\d+(?:\.\d+)?(?:ms|s|m|h|d)\b/,
        alias: 'number',
      },
      /\b\d+(?:\.\d+)?\b/,
    ],
    boolean: /\b(?:true|false|null)\b/,
    operator: /->|:=|=>/,
    punctuation: /[{}[\]();,.:?=]/,
    generic: {
      pattern: /<[^>]+>/,
      inside: {
        punctuation: /[<>[\].]/,
        'class-name': /[A-Za-z_]\w*/,
      },
    },
    interpolation: {
      pattern: /\{(?:[^{}]|\{[^{}]*\})*\}/,
      greedy: true,
      inside: {
        punctuation: /^[{}]|[{}]$/,
        expression: {
          pattern: /[\s\S]+/,
          inside: expression,
        },
      },
    },
  });
}
