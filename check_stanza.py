import stanza
print("Stanza version:", stanza.__version__)
from stanza.models.common.doc import Sentence
methods = [m for m in dir(Sentence) if "conll" in m.lower() or "serial" in m.lower() or "dump" in m.lower() or "to_" in m.lower()]
print("Relevant methods:", methods)