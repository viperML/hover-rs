
<h1>
  <p align="center">
    hover-rs - Protective home overlay
  </p>
</h1>

<h4>
  <p align="center">
    <i>Tired of programs messing your precious $HOME ? hover-rs is for you!</i>
  </p>
</h4>

<p align="center">
  <img src="./.github/Designer.jpeg" width="274">
</p>


---

hover-rs uses Linux's user namespaces to mount a volatile overlayfs over your
$HOME. Any write or delete operation is redirected to the upper layer, while
your $HOME is left intact. Read more in my blogpost: [https://ayats.org/blog/hover](https://ayats.org/blog/hover).

## Requirements

Your kernel must have user namespaces and overlayfs enabled:

```
$ zcat /proc/config.gz | grep -e NAMESPACES= -e USER_NS= -e OVERLAY_FS=
CONFIG_NAMESPACES=y
CONFIG_USER_NS=y
CONFIG_OVERLAY_FS=m
```

## Static binaries

CI builds statically-linked binaries (with musl). You can use download and use
them directly:

```
$ curl -OL https://github.com/viperML/hover-rs/releases/download/latest/hover-static-x86_64-linux
$ chmod +x hover-static-x86_64-linux
$ ./hover-static-x86_64-linux
```

## Building

With nix:

```
nix build github:viperML/hover-rs
```

With guix:

```
guix build -L .
```

If you bring your own cargo+rustc, just `cargo build`.

## Attribution

Inspired by
[https://github.com/max-privatevoid/hover](https://github.com/max-privatevoid/hover),
which uses fuse-overlayfs.
