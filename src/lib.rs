#[macro_export]
macro_rules! all_of {
    ($($x:expr),*) => ($crate::mappers::all_of($crate::vec_of_boxes![$($x),*]));
    ($($x:expr,)*) => ($crate::all_of![$($x),*]);
}

#[macro_export]
macro_rules! any_of {
    ($($x:expr),*) => ($crate::mappers::any_of($crate::vec_of_boxes![$($x),*]));
    ($($x:expr,)*) => ($crate::any_of![$($x),*]);
}

#[macro_export]
macro_rules! vec_of_boxes {
    ($($x:expr),*) => (std::vec![$(std::boxed::Box::new($x)),*]);
    ($($x:expr,)*) => ($crate::vec_of_boxes![$($x),*]);
}

pub mod mappers;
pub mod responders;
pub mod server;

pub type FullRequest = hyper::Request<Vec<u8>>;
pub type FullResponse = hyper::Response<Vec<u8>>;
pub use mappers::Matcher;

pub use server::Expectation;
pub use server::Server;
pub use server::Times;
