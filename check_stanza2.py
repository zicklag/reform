import stanza
# Check Document-level methods too
from stanza.models.common.doc import Document
methods = [m for m in dir(Document) if "conll" in m.lower() or "to_" in m.lower() or "serial" in m.lower()]
print("Document methods:", methods)

# Check if there's a CoNLL utility
from stanza.utils import conll
print("CoNLL utils:", [m for m in dir(conll) if not m.startswith("_")])