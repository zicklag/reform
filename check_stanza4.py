from stanza.utils.conll import CoNLL
import inspect
print("dict2conll:", inspect.signature(CoNLL.dict2conll))
print("write_doc2conll:", inspect.signature(CoNLL.write_doc2conll))
print("conll2dict:", inspect.signature(CoNLL.conll2dict))