"""Root-free policy preview: python profiles.py /absolute/path/to/saucepan."""
import sys
from saucepan_sdk import Client
from saucepan_sdk.models.setup import ExplainRequest

client = Client(executable=sys.argv[1])
print(client.profiles())
profile = client.profile('temporary')
request: ExplainRequest = {'operation': 'retrieve', 'request': {
    'source': {'kind': 'inline', 'source': {'type': 'url', 'url': 'https://example.invalid/archive.zip'}},
    'selector': {'type': 'url'}, 'policy_refs': profile['retrieval_policy_refs'],
}}
print(client.explain_policies(request, policies=profile['catalog']))
# To use temporary content, pass these refs/catalog to run_retrieved; the
# native child lifetime holds the files. A preview never fetches this URL.
