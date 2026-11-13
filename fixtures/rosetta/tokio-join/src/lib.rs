async fn prepare(statement: &'static str) -> Result<usize, &'static str> {
    if statement.is_empty() { Err("missing statement") } else { Ok(statement.len()) }
}

pub async fn prepare_batch() -> Result<[usize; 3], &'static str> {
    let (first, second, third) = tokio::try_join!(prepare("a"), prepare("b"), prepare("c"))?;
    Ok([first, second, third])
}

pub async fn write_batch() -> Result<(usize, usize), &'static str> {
    let written = tokio::join!(prepare("registry"), prepare("commands"));
    Ok((written.0?, written.1?))
}

pub async fn ordered() -> Result<(usize, usize), &'static str> {
    use tokio::try_join as gather;
    gather![/* Poll the shutdown check first */ biased; prepare("shutdown"), prepare("status"),]
}

pub async fn local() -> i32 {
    macro_rules! join { ($($token:tt)*) => { 1 }; }
    macro_rules! try_join { ($($token:tt)*) => { 2 }; }
    join!() + try_join!()
}

pub async fn join_all_control() -> i32 {
    async fn join_all() -> i32 { 1 }
    join_all().await
}
