quick_error! {
    #[derive(Debug)]
    pub enum AuthError {
        NetworkError(reason: &'static str) {}
    }
}
