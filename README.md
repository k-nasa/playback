# playback (wip)

## Overview

[![Actions Status](https://github.com/k-nasa/playback/workflows/CI/badge.svg)](https://github.com/k-nasa/playback/actions)

A tool for replaying access.

Resend the request that was actually sent based on the access log.
(Please use it when you want to send the request of the production environment to the sandbox.)

## Installation

wip

## expect log format

The following access logs are supported. Only json access log can be read.

```json
[
  {
    "accessed_at": "2020-06-22 04:24:00.678451 UTC", // Currently only supports utc
    "url": "http://localhost:8080",
    "http_method": "get",
    "http_header": {},
    "http_body": "hoge"
  }
]
```

## Usage

```console
playback --file log.json

playback --file log.json --shift 1d
```

## Contribution

1. Fork it ( http://github.com/k-nasa/playback )
2. Create your feature branch (git checkout -b my-new-feature)
3. Commit your changes (git commit -am 'Add some feature')
4. Push to the branch (git push origin my-new-feature)
5. Create new Pull Request

## License

[MIT](LICENSE)
