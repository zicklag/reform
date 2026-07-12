#!/bin/bash

set -e

# This is the best training setup I've found so far for inform-type language.
cargo r -- tagger train --max-epochs 2 data/en_childes-ud-train.conllu data/en_gum-ud-train.conllu
cargo r -- tagger eval data/en_childes-ud-test.conllu data/en_gum-ud-test.conllu
