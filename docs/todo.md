# Things to implement

## `b3`


## `data`

- [ ] FASTQ type/parser

- [ ] Tree builder

  - [ ] Rerooting
  - [ ] Conversion from Newick (see the Newick parser item)
  - [ ] Inline children lists via `smallvec` for better performance
  - [ ] `is_binary` method
  - [ ] Conversion to the binary tree

- [ ] Binary tree

  - [ ] [Branch score (Kuhner--Felsenstein) distance](https://doi.org/10.1093/oxfordjournals.molbev.a040126)

  - [ ] Basic SVG rendering

    - [ ] Adjustable scale
    - [ ] Configurable node/edge color via a passed closure

  - [ ] [Tidy tree rendering](https://doi.org/10.1093/molbev/msac204)
  - [ ] New Hampshire X format getter for node label metadata.  Should parse inside the method and return the value as an `&str` slice.
  - [ ] Compact and easy to parse binary format

- [ ] Python API for the builder and the binary tree

- [ ] New non-recursive Newick parser which parses straight to the tree builder

- [ ] Multifurcating tree which supports hybrid nodes (several parents, only one is canonical, the other is only supported via a second child).

- [ ] Streaming NEXUS file format parser

  - [ ] TAXA
  - [ ] CHARACTERS, parsed into an MSA type
  - [ ] DATA, same as above
  - [ ] Trees, streaming?
  - [ ] Distances (TBD matrix type)

- [ ] VCF

  - [ ] Types
  - [ ] VCF parser
  - [ ] BCF parser

- [ ] GFF

  - [ ] Types
  - [ ] GFF3 parser

- [ ] BED

- [ ] Multiple sequence alignment

  - [ ] Column score
  - [ ] Column/row views?
  - [ ] Mutable views?


## `linalg`

Remove when `computare` can replace it.
