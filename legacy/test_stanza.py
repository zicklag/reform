"""Quick test: parse Inform 7 assertion sentences with Stanza, output CoNLL-U."""
import os, ssl, tempfile
ssl._create_default_https_context = ssl._create_unverified_context

import requests
_old_get = requests.get
def _patched_get(*args, **kwargs):
    kwargs['verify'] = False
    return _old_get(*args, **kwargs)
requests.get = _patched_get

_old_session_request = requests.Session.request
def _patched_request(self, *args, **kwargs):
    kwargs['verify'] = False
    return _old_session_request(self, *args, **kwargs)
requests.Session.request = _patched_request

import urllib3
urllib3.disable_warnings(urllib3.exceptions.InsecureRequestWarning)

import stanza
from stanza.utils.conll import CoNLL

nlp = stanza.Pipeline(
    'en',
    model_dir='stanza_resources',
    processors='tokenize,mwt,pos,lemma,depparse',
    verbose=False,
)

sentences = [
    "The crate is a container.",
    "The crate is in the Gazebo.",
    "Mr Jones wears a top hat.",
    "North of the Sandy Beach is the Rocky Cove.",
    "A fruit is a kind of thing.",
]

for text in sentences:
    doc = nlp(text)
    tmp = tempfile.mktemp(suffix=".conllu")
    CoNLL.write_doc2conll(doc, tmp)
    with open(tmp) as f:
        print(f"# text = {text}")
        print(f.read())
    os.unlink(tmp)