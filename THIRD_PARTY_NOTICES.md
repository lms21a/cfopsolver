# Third-party notices

## rcuber 0.7.20

Parts of the private cube engine in `src/engine/` are derived from
[`rcuber`](https://github.com/adungaos/rcuber) 0.7.20 by Adun Gaos and rcuber
contributors. The extracted code is used under the MIT license declared by the
upstream package.

The extracted version corresponds to upstream commit
[`46d28b252c13f5e5c8e3d2f4ac98ec3db587cfba`](https://github.com/adungaos/rcuber/tree/46d28b252c13f5e5c8e3d2f4ac98ec3db587cfba).
Local changes include module extraction, pair-boundary F2L solving, deterministic
ordering, transition verification, and standard-library lazy table initialization.
The crate's MIT declaration is recorded in its Cargo metadata; upstream rcuber does
not ship a separate license file.

rcuber identifies [kociemba/kociema](https://github.com/adungaos/kociema) by Adun Gaos
(also declared MIT in its Cargo metadata), Kewb, PyCuber, and min2phase as sources.
The applicable notices for those retained portions are included below.

```text
MIT License

Copyright (c) Adun Gaos

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Kewb

Base cube representations are derived through rcuber/kociema from
[Kewb](https://github.com/luckasRanarison/kewb).
[Upstream license](https://github.com/luckasRanarison/kewb/blob/master/LICENSE):

```text
MIT License

Copyright (c) 2023 LIOKA Ranarison Fiderana

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## PyCuber

The CFOP implementation in rcuber credits
[PyCuber](https://github.com/adrianliaw/PyCuber).
[Upstream license](https://github.com/adrianliaw/PyCuber/blob/master/LICENSE):

```text
The MIT License (MIT)

Copyright (c) 2014 Adrian Liaw

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## min2phase

The two-phase implementation is ported through rcuber from
[min2phase by Shuang Chen](https://github.com/cs0x7f/min2phase).
We use its MIT option, added upstream in
[`4d183b9eff8119cac72bc50ef35a7d8990740e06`](https://github.com/cs0x7f/min2phase/commit/4d183b9eff8119cac72bc50ef35a7d8990740e06).
The following is the MIT notice from its README:

```text
Copyright (c) 2023 Chen Shuang

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

Cargo dependencies retain their own licenses and are identified in `Cargo.lock`.
Their full license texts are bundled in `THIRD_PARTY_DEPENDENCIES.txt`. Maturin also
embeds a dependency SBOM in the Python wheel. All notices are shipped with the
crate, source distribution, and wheel.
