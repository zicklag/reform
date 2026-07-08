"""Download Stanza models with SSL verification disabled (proxy workaround).

The proxy's CA cert is missing the keyUsage extension, which Python 3.14's
OpenSSL rejects. We work around this by:
1. Monkey-patching requests to disable SSL verification
2. Passing proxies to stanza.download() so it uses requests (not huggingface_hub/httpx)
"""
import os
import ssl
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

# Pass proxies so Stanza uses requests (patched) instead of huggingface_hub (httpx)
proxies = {
    "http": os.environ.get("http_proxy"),
    "https": os.environ.get("https_proxy"),
}

model_dir = os.path.expanduser("~/git/zicklag/reform/stanza_resources")

import stanza
stanza.download('en', model_dir=model_dir, proxies=proxies)
print('Download complete!')
print('Models saved to:', model_dir)