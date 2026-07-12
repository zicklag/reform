#!/bin/bash

set -e

# This is the best training setup I've found so far for inform-type language.
# 94.29% accuracy on CHILDS.
cargo r -- tagger train --max-epochs 1 data/en_childes-ud-*.conllu
# cargo r -- tagger eval data/en_childes-ud-test.conllu