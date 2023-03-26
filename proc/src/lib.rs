#![deny(clippy::pedantic)]
#![warn(clippy::nursery, clippy::all)]

use proc_macro::TokenStream;

mod requests;

#[proc_macro]
pub fn requests(item: TokenStream) -> TokenStream {
    requests::requests(item)
}
