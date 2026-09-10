import { Saucepan, type Acquired, type AppToken, type AppView, type Recipe, type SourceState } from '../src/index.js';
const client = new Saucepan({ app: 'consumer' });
const recipes: Recipe[] = [
  { source: { provider: 'git', origin: 'https://example.com/repo.git', reference: 'main' }, folder: 'assets' },
  { source: { provider: 'url', url: 'https://example.com/file', download: { format: 'file', name: 'file' } } },
  { source: { provider: 'url', url: 'https://example.com/archive.zip', download: { format: 'zip' } } },
  { source: { provider: 'local', path: './assets' } },
];
const acquired: Promise<Acquired> = client.acquire(recipes[0]!);
const view: Promise<AppView> = client.view();
const token: Promise<AppToken> = client.register('consumer');
const history: Promise<SourceState | null> = client.history('source-id');
const path: Promise<string | null> = client.path('artifact-id');
void [acquired, view, token, history, path];
// @ts-expect-error Git recipes require a requested ref.
client.acquire({ source: { provider: 'git', origin: 'https://example.com/repo.git' } });
// @ts-expect-error Both explicit test-store fields are required.
new Saucepan({ testStore: { root: './store' } });
// @ts-expect-error Filters select known providers; they are not permissions.
client.configure({ filters: { source_ids: [], providers: ['admin'] } });
