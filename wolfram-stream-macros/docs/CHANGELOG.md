# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

* Initial release: `#[derive(InputStream)]`, `#[derive(SeekableInputStream)]`
  and `#[derive(OutputStream)]`, which forward the `wolfram-library-link` stream
  traits to a type's existing `std::io::Read` / `Write` / `Seek`
  implementations.
