from stanza.utils.conll import CoNLL
print("CoNLL methods:", [m for m in dir(CoNLL) if not m.startswith("_")])