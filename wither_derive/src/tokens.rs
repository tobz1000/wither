use std::str::FromStr;

use quote::{TokenStreamExt, ToTokens};
use proc_macro2::{TokenTree, Spacing, Span, Punct, TokenStream};

use mongodb::coll::options::{IndexModel, IndexOptions};

pub struct Indexes(pub Vec<IndexModel>);

impl ToTokens for Indexes {
    /// Implement `ToTokens` for the `Indexes` type.
    ///
    /// This type is a simple wrapper around a `Vec<IndexModel>` and when it is converted to a
    /// token stream, it will simply be returned as the underlying `Vec` type.
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(TokenStream::from_str(r#"vec!"#).unwrap().into_iter());
        tokens.append(Punct::new('[', Spacing::Joint));

        // // Iterate over each index model and generate needed token stream.
        // for idx in self.0.iter() {
        //     tokens.extend(TokenStream::from_str(r#"IndexModel{
        //         keys:
        //     }"#).unwrap().into_iter());
        // }

        tokens.append(Punct::new(']', Spacing::Joint));
    }
}
