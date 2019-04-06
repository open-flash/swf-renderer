<a href="https://github.com/open-flash/open-flash">
    <img src="https://raw.githubusercontent.com/open-flash/open-flash/master/logo.png"
    alt="Open Flash logo" title="Open Flash" align="right" width="64" height="64" />
</a>

# SWF Renderer

[![npm](https://img.shields.io/npm/v/swf-renderer.svg?maxAge=86400)](https://www.npmjs.com/package/swf-renderer)
[![crates.io](https://img.shields.io/crates/v/swf-renderer.svg?maxAge=86400)](https://crates.io/crates/swf-renderer)
[![GitHub repository](https://img.shields.io/badge/Github-open--flash%2Fswf--renderer-blue.svg?maxAge=86400)](https://github.com/open-flash/swf-renderer)
[![Build status](https://img.shields.io/travis/open-flash/swf-renderer/master.svg?maxAge=86400)](https://travis-ci.org/open-flash/swf-renderer)

SWF renderer implemented in Rust and Typescript (Node and browser).
Converts shapes to pixels.

- [Rust implementation](./rs/README.md)
- [Typescript implementation](./ts/README.md)

This library is part of the [Open Flash][ofl] project.

## Usage

- [Rust](./rs/README.md#usage)
- [Typescript](./ts/README.md#usage)

## Status

The Typescript implementation has Node and browser based on the
`CanvasRendering2D` backend. It can decode shapes and morph and render
gradients and solid fill styles. It has basic support for textures.

The Rust implementation is merely experimental. It uses `gfx-rs` to renderer
the shapes using the GPU.

## Contributing

- [Rust](./rs/README.md#contributing)
- [Typescript](./ts/README.md#contributing)

You can also use the library and report any issues you encounter on the Github
issues page.

[ofl]: https://github.com/open-flash/open-flash
[swf-tree]: https://github.com/open-flash/swf-tree
