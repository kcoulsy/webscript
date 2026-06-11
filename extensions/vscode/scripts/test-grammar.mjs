import {readFile} from 'node:fs/promises';
import {join} from 'node:path';

const root = new URL('..', import.meta.url).pathname;
const grammarPath = join(root, 'syntaxes', 'webscript.tmLanguage.json');
const grammar = JSON.parse(await readFile(grammarPath, 'utf8'));

const requiredRepositoryKeys = [
  'comments',
  'strings',
  'numbers-and-durations',
  'types',
  'expressions',
  'line-directives',
  'block-directives',
  'model-field',
  'schema-field',
  'model-index',
  'model-decorator',
  'schema-decorator',
  'markup',
  'style-block'
];

for (const key of requiredRepositoryKeys) {
  assert(grammar.repository?.[key], `missing repository key: ${key}`);
}

assert(grammar.scopeName === 'source.webscript', 'unexpected scopeName');
assert(grammar.fileTypes?.includes('web'), 'missing .web file type');

const fixtureNames = [
  'page.web',
  'component.web',
  'button.web',
  'event-demo.web',
  'model.web',
  'schema.web'
];
const fixtures = await Promise.all(
  fixtureNames.map((name) => readFile(join(root, 'test', 'fixtures', name), 'utf8'))
);
const source = fixtures.join('\n\n');

const snippets = [
  /@page\s+"\/"/,
  /@component\s+Counter\s+\{/,
  /@component\s+UI\.Button\s+\{/,
  /@component\s+EventDemo\s+\{\}/,
  /@model\s+Todo\s+\{/,
  /id:\s*int\s+@primary\s+@auto/,
  /done:\s*bool\s+@default\(false\)/,
  /authorId:\s*int\s+@references\(User\.id\)\s+@relation\(author\)/,
  /@index\(done,\s*createdAt\)/,
  /@uniqueIndex\(title\)/,
  /@schema\s+AddTodoInput\s+\{/,
  /title:\s*string\s+@min\(1\)\s+@max\(120\)/,
  /assigneeEmail:\s*string\s+@optional\s+@email/,
  /@load\s+\{/,
  /@action\s+rememberName\(input:\s*AddTodoInput\)\s+\{/,
  /@client\s+\{/,
  /fn\s+save\(\)\s+\{/,
  /count:\s*signal<int>\s*=\s*initial/,
  /note:\s*signal<string>\s*=\s*""/,
  /@click=\{count\+\+\}/,
  /<UI\.Button\b/,
  /value=\{session\.name\}|featured=\{post\.featured\}/,
  /@if\s+ready\s+&&\s+!false\s+\{/,
  /\}\s+@else\s+\{/,
  /@for\s+post\s+in\s+posts\s+\{/,
  /@style(?:\s+scoped)?\s+\{/,
  /@media\s+\(min-width:\s*600px\)\s+\{/
];

for (const snippet of snippets) {
  assert(snippet.test(source), `fixture is missing representative snippet: ${snippet}`);
}

const grammarText = await readFile(grammarPath, 'utf8');
const grammarSnippets = [
  /"scopeName":\s*"source\.webscript"/,
  /"contentName":\s*"text\.html\.basic"/,
  /"contentName":\s*"source\.css"/,
  /"contentName":\s*"source\.webscript\.expression"/,
  /"include":\s*"source\.css"/,
  /@\(\?:if\|for\|do\)/,
  /@style/,
  /@\[A-Za-z\]\[A-Za-z0-9_-\]\*/
];

for (const snippet of grammarSnippets) {
  assert(snippet.test(grammarText), `grammar is missing expected pattern: ${snippet}`);
}

console.log(`Grammar smoke test passed for ${fixtureNames.length} fixtures.`);

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}
