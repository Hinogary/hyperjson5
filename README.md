# hyperjson5

A hyper-fast, safe Python module to read and write JSON data. Works as a
drop-in replacement for Python's built-in
[json](https://docs.python.org/3/library/json.html) module.
This is alpha software and there will be bugs, so maybe don't deploy to production _just_ yet. :wink:

## Installation

```
pip install hyperjson5
```

## Usage

hyperjson5 is meant as a drop-in replacement for Python's [json
module](https://docs.python.org/3/library/json.html):

```python
>>> import hyperjson5
>>> hyperjson5.dumps([{"key": "value"}, 81, True])
'[{"key":"value"},81,true]'
>>> hyperjson5.loads("""[{key: "value"}, 81, true]""")
[{u'key': u'value'}, 81, True]
```

## Contributions welcome!

## License

hyperjson5 is licensed under either of

- Apache License, Version 2.0, (LICENSE-APACHE or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in hyperjson by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
