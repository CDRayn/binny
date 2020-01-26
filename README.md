binny
=======

A general purpose library for parsing common binary file formats such as mp3, wav, jpeg, etc.

## About

`binny` is used to parse and valid common binary file formats from a supplied `Read` trait.
The file is either parsed into a struct that represents the physical structure of the file, such as
a file header, meta data, or contents of the file used for encoding information assuming a valid file
is being parsed. If the file being parsed is invalid, a enumerated error is returned that details any
violations of said file's format.

`binny` is not a decoder, rather it handles the step of parsing a file that is a prerequisite of
decoding. The task of decoding a file is left up to other libraries or methods.

## Usage
 
 To use `binny` in your project, simply add `binny` to your project's `Cargo.toml` file like so:
 
 ```toml
[dependencies]
binny = "0.1.0" 
```

## Roadmap

The following is the list of features and functionality on the roadmap for `binny`'s development:

* `mp3` parsing and validation support
* tutorial section in the project's documentation
* `wav` parsing and validation support
* `flac` parsing and validation support
* `jpeg` parsing and validation support
* `png` parsing and validation support
* `gif` parsing and validation support
* `tiff` parsing and validation support
* `bmp` parsing and validation

## License

`binny` is free and open source software licensed under the MIT license. For further details please refer to the
`LICENSE.txt` file or [here](https://opensource.org/licenses/MIT).

## How to Contribute

### Compatibility

`binny` takes semantic version seriously and so many of the guidelines and policies surrounding contributing are
focused on avoiding braking changes.

`binny` will pin the minimum required version of Rust to the CI builds. This means that bumping the minimum required 
version Rust will result in the *minor* version being bumped since a requirement for a newer version of Rust is a
breaking change. 

`binny` will officially support the current stable version of Rust and the two previous releases but it may be 
compatibility with prior releases is not guaranteed.

### Documentation

All contributions are expected to sufficiently documented. This a docstring for all callables, modules, and traits. All
contributions are expected to have accompanying unit tests.

### Git Branching

This project follows the GitFlow branching strategy. In short the `master` branch should always match a tagged release 
of the project. The `development` branch should only contain the aggregate of development that is staged for the next 
release. The `master` and `development` branches should never be committed to directly, rather than should be merged
into via a pull request. Changes should only be directly committed via a `feature` branch for new features, an 
`enhancement` branch for enhancements to existing features, or via a `bugfix` branch for bug fixes. See the this 
[blog post](https://nvie.com/posts/a-successful-git-branching-model/) for a more detailed explanation on GitFlow.
  