# hover-rs

Tired of programs messing your precious $HOME ? hover-rs is for you!

---

hover-rs uses Linux's user namespaces to mount a volatile overlayfs over your $HOME. Any write or delete operation is redirected to the upper layer, while your $HOME is left intact.

## Requirements

Your kernel must have namespaces enabled:

```
$ zcat /proc/config.gz | rg CONFIG_NAMESPACES
CONFIG_NAMESPACES=y
```

(probably more hidden requirements...)

## Building

```
nix build github:viperML/hover-rs
```
