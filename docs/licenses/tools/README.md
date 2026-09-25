# Synthetic archive-fixture tools

These packages are isolated test-time tools used only by
`scripts/make-archive-fixtures.py`. They are not application or release
runtime dependencies. The setup is intentionally local and does not install
anything globally:

```powershell
python -m pip install --no-cache-dir --no-compile --target .local/tools/pyzipper-0.4.0 pyzipper==0.4.0 pycryptodomex==3.23.0
```

The dependency is imported from `.local/tools/pyzipper-0.4.0`. The generated
archives contain fixed synthetic values and the fixed test password
`PUBLIC_TEST_PASSWORD`; neither is a production-backup credential.

## Pinned source and hashes

The package metadata and wheel hashes were checked against the official PyPI
JSON endpoints:

- pyzipper 0.4.0: <https://pypi.org/pypi/pyzipper/0.4.0/json>
  - wheel: `pyzipper-0.4.0-py3-none-any.whl`
  - SHA-256: `aa7b8a0fe741d67aac36ead85f6e735af107b72f84e0775f2ed565fc0d3a2f02`
  - source: <https://github.com/danifus/pyzipper/tree/0.4.0>
- pycryptodomex 3.23.0: <https://pypi.org/pypi/pycryptodomex/3.23.0/json>
  - wheel: `pycryptodomex-3.23.0-cp37-abi3-win_amd64.whl`
  - SHA-256: `52e5ca58c3a0b0bd5e100a9fbc8015059b05cffc6c66ce9d98b4b45e023443b9`
  - source: <https://github.com/Legrandin/pycryptodome/tree/v3.23.0>

The cached pyzipper distribution also carries Python's PSF license notice in
`LICENSE.python`; it is not application code and the helper is not shipped as
a runtime component. The copied package license texts are retained alongside
this note: [pyzipper MIT](pyzipper-0.4.0-LICENSE.txt), [Python PSF notice
bundled by pyzipper](pyzipper-0.4.0-LICENSE.python.txt), and
[pycryptodomex BSD-2-Clause/public-domain](pycryptodomex-3.23.0-LICENSE.rst).

The source distribution license hashes are `A61B10433D33778E1FE918492991AA4111C2AB0F7370ADEB083C4E079941338D`
(pyzipper MIT), `BFF9147E9E26FEE7201BCA8CD46147713C4F5E10A73B87AF160A43B118F62DCE`
(pyzipper's Python notice), and
`60B8958A9EF9B7EC512087B725555372175ED2B02B969F8725B8534FDE48ACDD`
(pycryptodomex). The checked-in copies preserve the license text. The MIT
copy uses LF, the Python notice retains CRLF, and the pycryptodomex copy uses
LF instead of the package's CRLF; their resulting hashes are respectively
`A61B10433D33778E1FE918492991AA4111C2AB0F7370ADEB083C4E079941338D`,
`BFF9147E9E26FEE7201BCA8CD46147713C4F5E10A73B87AF160A43B118F62DCE`, and
`4E04660D77C1C64E89D79537919FB8240FA21484A7E3DB29F358B2C7F84EA073`.
