//! Codegen for the stream derives.
//!
//! The emitted code names items under `::wolfram_library_link::stream::*`.
//! Unlike `wolfram-export-macros`, which has to choose between two possible
//! host crates at expansion time, the streams API lives in exactly one crate,
//! so the path is named directly.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::DeriveInput;

/// Which stream trait to generate, and whether to wire up seeking.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Input,
    SeekableInput,
    Output,
}

pub(crate) fn expand(input: &DeriveInput, kind: Kind) -> TokenStream2 {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    match kind {
        Kind::Input => quote! {
            impl #impl_generics ::wolfram_library_link::stream::InputStream for #name #ty_generics
                #where_clause
            {
                fn read(
                    &mut self,
                    buf: &mut [u8],
                ) -> ::std::result::Result<usize, ::wolfram_library_link::stream::StreamError> {
                    ::std::result::Result::Ok(::std::io::Read::read(self, buf)?)
                }
            }
        },

        Kind::SeekableInput => quote! {
            impl #impl_generics ::wolfram_library_link::stream::InputStream for #name #ty_generics
                #where_clause
            {
                fn read(
                    &mut self,
                    buf: &mut [u8],
                ) -> ::std::result::Result<usize, ::wolfram_library_link::stream::StreamError> {
                    ::std::result::Result::Ok(::std::io::Read::read(self, buf)?)
                }

                fn seek(
                    &mut self,
                    offset: i64,
                ) -> ::std::result::Result<(), ::wolfram_library_link::stream::StreamError> {
                    // The Wolfram Language only ever asks for an absolute
                    // position, and never a negative one.
                    ::std::io::Seek::seek(
                        self,
                        ::std::io::SeekFrom::Start(::std::cmp::max(offset, 0) as u64),
                    )?;
                    ::std::result::Result::Ok(())
                }

                fn is_seekable(&self) -> bool {
                    true
                }

                fn tell(
                    &mut self,
                ) -> ::std::result::Result<i64, ::wolfram_library_link::stream::StreamError> {
                    ::std::result::Result::Ok(
                        ::std::io::Seek::stream_position(self)? as i64
                    )
                }

                fn size(
                    &mut self,
                ) -> ::std::result::Result<i64, ::wolfram_library_link::stream::StreamError> {
                    // `Seek::stream_len` is unstable, so measure by hand and put
                    // the position back.
                    let original = ::std::io::Seek::stream_position(self)?;
                    let size = ::std::io::Seek::seek(self, ::std::io::SeekFrom::End(0))?;
                    ::std::io::Seek::seek(
                        self,
                        ::std::io::SeekFrom::Start(original),
                    )?;
                    ::std::result::Result::Ok(size as i64)
                }
            }

            impl #impl_generics ::wolfram_library_link::stream::SeekableInputStream for #name #ty_generics
                #where_clause
            {
            }
        },

        Kind::Output => quote! {
            impl #impl_generics ::wolfram_library_link::stream::OutputStream for #name #ty_generics
                #where_clause
            {
                fn write(
                    &mut self,
                    buf: &[u8],
                ) -> ::std::result::Result<usize, ::wolfram_library_link::stream::StreamError> {
                    ::std::result::Result::Ok(::std::io::Write::write(self, buf)?)
                }

                fn flush(
                    &mut self,
                ) -> ::std::result::Result<(), ::wolfram_library_link::stream::StreamError> {
                    ::std::io::Write::flush(self)?;
                    ::std::result::Result::Ok(())
                }

                fn close(
                    mut self,
                ) -> ::std::result::Result<(), ::wolfram_library_link::stream::StreamError> {
                    ::std::io::Write::flush(&mut self)?;
                    ::std::result::Result::Ok(())
                }
            }
        },
    }
}
